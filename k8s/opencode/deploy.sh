#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHART_DIR="${SCRIPT_DIR}/helm/opencode"
NAMESPACE="xy"
RELEASE_NAME="opencode"
DOMAIN="opencode.xiaoyxq.top"
ENABLE_INGRESS="${ENABLE_INGRESS:-true}"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_prerequisites() {
    log_info "check prerequisites"
    
    if ! command -v helm &> /dev/null; then
        log_error "helm not found, please install helm first"
        exit 1
    fi
    
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl not found, please install kubectl first"
        exit 1
    fi
    
    if ! kubectl cluster-info &> /dev/null; then
        log_error "cannot connect to kubernetes cluster"
        exit 1
    fi
    
    log_info "prerequisites check passed"
}

create_namespace() {
    log_info "check namespace: ${NAMESPACE}"
    
    if kubectl get namespace "${NAMESPACE}" &> /dev/null; then
        log_warn "namespace ${NAMESPACE} already exists"
    else
        log_info "create namespace: ${NAMESPACE}"
        kubectl create namespace "${NAMESPACE}"
    fi
}

install_or_upgrade() {
    log_info "deploy ${RELEASE_NAME}"
    
    local HELM_ARGS=(
        "--namespace" "${NAMESPACE}"
        "--timeout" "5m"
        "--wait"
    )
    
    if [ "${ENABLE_INGRESS}" = "true" ]; then
        log_info "ingress will be enabled"
        HELM_ARGS+=(
            "--set" "ingress.enabled=true"
            "--set" "ingress.className=traefik"
            "--set" "ingress.hosts[0].host=${DOMAIN}"
            "--set" "ingress.hosts[0].paths[0].path=/"
            "--set" "ingress.hosts[0].paths[0].pathType=Prefix"
        )
    fi
    
    if helm list -n "${NAMESPACE}" | grep -q "^${RELEASE_NAME}"; then
        log_info "upgrade release: ${RELEASE_NAME}"
        helm upgrade "${RELEASE_NAME}" "${CHART_DIR}" "${HELM_ARGS[@]}"
    else
        log_info "install release: ${RELEASE_NAME}"
        helm install "${RELEASE_NAME}" "${CHART_DIR}" "${HELM_ARGS[@]}"
    fi
    
    log_info "deployment completed"
}

verify_deployment() {
    log_info "verify deployment status"
    
    kubectl rollout status deployment/"${RELEASE_NAME}" -n "${NAMESPACE}" --timeout=5m
    
    log_info "pod status:"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=opencode,app.kubernetes.io/instance=${RELEASE_NAME}"
    
    log_info "service status:"
    kubectl get svc -n "${NAMESPACE}" -l "app.kubernetes.io/name=opencode,app.kubernetes.io/instance=${RELEASE_NAME}"
    
    if [ "${ENABLE_INGRESS}" = "true" ]; then
        verify_ingress
    fi
}

verify_ingress() {
    log_info "verify ingress status"
    
    sleep 2
    
    if kubectl get ingress "${RELEASE_NAME}" -n "${NAMESPACE}" &> /dev/null; then
        log_info "ingress created successfully"
        echo ""
        kubectl get ingress "${RELEASE_NAME}" -n "${NAMESPACE}"
        echo ""
    fi
}

show_info() {
    log_info "deployment information:"
    echo "  Release: ${RELEASE_NAME}"
    echo "  Namespace: ${NAMESPACE}"
    echo "  Chart: ${CHART_DIR}"
    
    if [ "${ENABLE_INGRESS}" = "true" ]; then
        echo "  Domain: ${DOMAIN}"
        echo ""
        log_info "access url:"
        echo "  http://${DOMAIN}"
    fi
    
    echo ""
    log_info "useful commands:"
    echo "  查看 pods: kubectl get pods -n ${NAMESPACE}"
    echo "  查看 logs: kubectl logs -f -n ${NAMESPACE} -l app.kubernetes.io/name=opencode"
    echo "  查看 service: kubectl get svc -n ${NAMESPACE}"
    
    if [ "${ENABLE_INGRESS}" = "true" ]; then
        echo "  查看 ingress: kubectl get ingress -n ${NAMESPACE}"
        echo "  禁用 ingress: ENABLE_INGRESS=false ./deploy.sh"
    else
        echo "  启用 ingress: ENABLE_INGRESS=true ./deploy.sh"
    fi
    
    echo "  卸载: helm uninstall ${RELEASE_NAME} -n ${NAMESPACE}"
}

main() {
    log_info "start deploy opencode"
    
    check_prerequisites
    create_namespace
    install_or_upgrade
    verify_deployment
    show_info
    
    log_info "deployment process finished successfully"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
