//! Tower fetcher for fetch node execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::task::Poll;

use apollo_compiler::validation::Valid;
use futures::future::BoxFuture;
use tower::BoxError;
use tower::ServiceExt;

use super::connector_service::ConnectorServiceFactory;
use super::fetch::BoxService;
use super::new_service::ServiceFactory;
use super::ConnectRequest;
use super::SubgraphRequest;
use crate::graphql;
use crate::graphql::Request as GraphQLRequest;
use crate::http_ext;
use crate::plugins::subscription::SubscriptionConfig;
use crate::query_planner::build_operation_with_aliasing;
use crate::query_planner::fetch::FetchNode;
use crate::services::FetchRequest;
use crate::services::FetchResponse;
use crate::services::SubgraphServiceFactory;
use crate::spec::Schema;

#[derive(Clone)]
pub(crate) struct FetchService {
    pub(crate) subgraph_service_factory: Arc<SubgraphServiceFactory>,
    pub(crate) schema: Arc<Schema>,
    pub(crate) subgraph_schemas: Arc<HashMap<String, Arc<Valid<apollo_compiler::Schema>>>>,
    pub(crate) _subscription_config: Option<SubscriptionConfig>, // TODO: add subscription support to FetchService
    pub(crate) connector_service_factory: Arc<ConnectorServiceFactory>,
}

impl tower::Service<FetchRequest> for FetchService {
    type Response = FetchResponse;
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: FetchRequest) -> Self::Future {
        let FetchRequest {
            fetch_node: FetchNode {
                ref service_name, ..
            },
            ..
        } = request;
        if self
            .connector_service_factory
            .connectors_by_service_name
            .contains_key(service_name.to_string().as_str())
        {
            Self::fetch_with_connector_service(
                self.schema.clone(),
                self.connector_service_factory.clone(),
                request,
            )
        } else {
            Self::fetch_with_subgraph_service(
                self.schema.clone(),
                self.subgraph_service_factory.clone(),
                self.subgraph_schemas.clone(),
                request,
            )
        }
    }
}

impl FetchService {
    fn fetch_with_connector_service(
        schema: Arc<Schema>,
        connector_service_factory: Arc<ConnectorServiceFactory>,
        request: FetchRequest,
    ) -> BoxFuture<'static, Result<FetchResponse, BoxError>> {
        let FetchRequest {
            fetch_node,
            supergraph_request,
            variables,
            context,
            current_dir,
            ..
        } = request;

        let FetchNode {
            operation,
            service_name,
            requires,
            output_rewrites,
            ..
        } = fetch_node;

        let paths = variables.inverted_paths.clone();

        Box::pin(async move {
            let (data, errors) = connector_service_factory
                .create()
                .oneshot(
                    ConnectRequest::builder()
                        .service_name(service_name.clone())
                        .context(context)
                        .operation_str(operation.to_string())
                        .supergraph_request(supergraph_request)
                        .variables(variables)
                        .build(),
                )
                .await?;

            let response = graphql::Response::builder()
                .data(data)
                .errors(errors)
                .build();

            let (value, errors) = FetchNode::response_at_path(
                &schema,
                &current_dir,
                paths,
                response,
                &requires,
                &output_rewrites,
                &service_name,
            );
            Ok((value, errors))
        })
    }

    fn fetch_with_subgraph_service(
        schema: Arc<Schema>,
        subgraph_service_factory: Arc<SubgraphServiceFactory>,
        subgraph_schemas: Arc<HashMap<String, Arc<Valid<apollo_compiler::Schema>>>>,
        request: FetchRequest,
    ) -> BoxFuture<'static, Result<FetchResponse, BoxError>> {
        let FetchRequest {
            fetch_node,
            supergraph_request,
            deferred_fetches,
            variables,
            current_dir,
            context,
        } = request;

        let FetchNode {
            service_name,
            operation,
            operation_kind,
            operation_name,
            requires,
            output_rewrites,
            id,
            ..
        } = fetch_node;

        let uri = schema
            .subgraph_url(service_name.as_ref())
            .unwrap_or_else(|| {
                panic!("schema uri for subgraph '{service_name}' should already have been checked")
            })
            .clone();

        let alias_query_string; // this exists outside the if block to allow the as_str() to be longer lived
        let aliased_operation = if let Some(ctx_arg) = &variables.contextual_arguments {
            if let Some(subgraph_schema) = subgraph_schemas.get(&service_name.to_string()) {
                match build_operation_with_aliasing(&operation, ctx_arg, subgraph_schema) {
                    Ok(op) => {
                        alias_query_string = op.serialize().no_indent().to_string();
                        alias_query_string.as_str()
                    }
                    Err(errors) => {
                        tracing::debug!(
                            "couldn't generate a valid executable document? {:?}",
                            errors
                        );
                        operation.as_serialized()
                    }
                }
            } else {
                tracing::debug!(
                    "couldn't find a subgraph schema for service {:?}",
                    &service_name
                );
                operation.as_serialized()
            }
        } else {
            operation.as_serialized()
        };

        let aqs = aliased_operation.to_string(); // TODO
        let sns = service_name.clone();
        let current_dir = current_dir.clone();
        let deferred_fetches = deferred_fetches.clone();
        let service = subgraph_service_factory
            .create(&sns)
            .expect("we already checked that the service exists during planning; qed");

        let mut subgraph_request = SubgraphRequest::builder()
            .supergraph_request(supergraph_request.clone())
            .subgraph_request(
                http_ext::Request::builder()
                    .method(http::Method::POST)
                    .uri(uri)
                    .body(
                        GraphQLRequest::builder()
                            .query(aliased_operation)
                            .and_operation_name(operation_name.as_ref().map(|n| n.to_string()))
                            .variables(variables.variables.clone())
                            .build(),
                    )
                    .build()
                    .expect("it won't fail because the url is correct and already checked; qed"),
            )
            .subgraph_name(service_name.to_string())
            .operation_kind(operation_kind)
            .context(context.clone())
            .build();
        subgraph_request.query_hash = fetch_node.schema_aware_hash.clone();
        subgraph_request.authorization = fetch_node.authorization.clone();
        Box::pin(async move {
            Ok(FetchNode::subgraph_fetch(
                service,
                subgraph_request,
                &sns,
                &current_dir,
                &requires,
                &output_rewrites,
                &schema,
                variables.inverted_paths,
                id,
                &deferred_fetches,
                &aqs,
                variables.variables,
            )
            .await)
        })
    }
}

#[derive(Clone)]
pub(crate) struct FetchServiceFactory {
    pub(crate) schema: Arc<Schema>,
    pub(crate) subgraph_schemas: Arc<HashMap<String, Arc<Valid<apollo_compiler::Schema>>>>,
    pub(crate) subgraph_service_factory: Arc<SubgraphServiceFactory>,
    pub(crate) subscription_config: Option<SubscriptionConfig>,
    pub(crate) connector_service_factory: Arc<ConnectorServiceFactory>,
}

impl FetchServiceFactory {
    pub(crate) fn new(
        schema: Arc<Schema>,
        subgraph_schemas: Arc<HashMap<String, Arc<Valid<apollo_compiler::Schema>>>>,
        subgraph_service_factory: Arc<SubgraphServiceFactory>,
        subscription_config: Option<SubscriptionConfig>,
        connector_service_factory: Arc<ConnectorServiceFactory>,
    ) -> Self {
        Self {
            subgraph_service_factory,
            subgraph_schemas,
            schema,
            subscription_config,
            connector_service_factory,
        }
    }

    pub(crate) fn subgraph_service_for_subscriptions(
        &self,
        service_name: &str,
    ) -> Option<crate::services::subgraph::BoxService> {
        self.subgraph_service_factory.create(service_name)
    }
}

impl ServiceFactory<FetchRequest> for FetchServiceFactory {
    type Service = BoxService;

    fn create(&self) -> Self::Service {
        FetchService {
            subgraph_service_factory: self.subgraph_service_factory.clone(),
            schema: self.schema.clone(),
            subgraph_schemas: self.subgraph_schemas.clone(),
            _subscription_config: self.subscription_config.clone(),
            connector_service_factory: self.connector_service_factory.clone(),
        }
        .boxed()
    }
}