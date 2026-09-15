import { useInvestigationStore } from "../investigation/investigation-store";
import {
  runCloseWorkflow,
  runGetWorkflow,
  runNextStep,
  runStartWorkflow,
  type CloseWorkflowResult,
  type StartWorkflowParams,
  type WorkflowBridgeResult,
  type WorkflowStepResult,
} from "./workflow-bridge-client";
import type {
  WorkflowKindId,
  WorkspaceRequestContext,
  WorkspaceWorkflowBinding,
} from "./workflow-types";

export type WorkspaceWorkflowResult<T = WorkflowStepResult> =
  | WorkflowBridgeResult<T>
  | { status: "stale" }
  | { status: "idle" }
  | { status: "incompatible" };

function stepFromBinding(binding: WorkspaceWorkflowBinding): WorkflowStepResult {
  return {
    workflowId: binding.workflowId,
    currentStep: binding.currentStep,
    stepResult: null,
    nextExpectedInput: [],
  };
}

function finishNonReadyRequest(context: WorkspaceRequestContext) {
  useInvestigationStore.getState().finishWorkspaceRequest("workflow", context);
}

export async function closeOrDetachWorkspaceWorkflow(): Promise<void> {
  const detached = useInvestigationStore.getState().detachWorkflow();
  if (!detached) {
    return;
  }
  await runCloseWorkflow(detached.workflowId);
}

export async function startWorkspaceWorkflow(
  kind: WorkflowKindId,
  params?: StartWorkflowParams,
): Promise<WorkspaceWorkflowResult> {
  const context = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
  const detached = useInvestigationStore.getState().detachWorkflow();
  if (detached) {
    await runCloseWorkflow(detached.workflowId);
    if (!useInvestigationStore.getState().acceptWorkspaceRequest("workflow", context)) {
      return { status: "stale" };
    }
  }
  const result = await runStartWorkflow(kind, params);
  if (result.status !== "ready") {
    finishNonReadyRequest(context);
    return result;
  }
  if (!useInvestigationStore.getState().bindWorkflow(context, kind, result.data)) {
    return { status: "stale" };
  }
  return result;
}

export async function advanceWorkspaceWorkflow(
  input?: unknown,
): Promise<WorkspaceWorkflowResult> {
  const active = useInvestigationStore.getState().activeWorkflow;
  if (!active || useInvestigationStore.getState().workflowNeedsRecovery) {
    return { status: "idle" };
  }
  const context = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
  const result = await runNextStep(active.workflowId, input);
  if (result.status !== "ready") {
    finishNonReadyRequest(context);
    return result;
  }
  if (
    result.data.workflowId !== active.workflowId ||
    !useInvestigationStore.getState().bindWorkflow(context, active.kind, result.data)
  ) {
    return { status: "stale" };
  }
  return result;
}

export async function recoverWorkspaceWorkflow(): Promise<WorkspaceWorkflowResult> {
  const active = useInvestigationStore.getState().activeWorkflow;
  if (!active) {
    return { status: "idle" };
  }
  if (!useInvestigationStore.getState().workflowNeedsRecovery) {
    return { status: "ready", data: stepFromBinding(active) };
  }

  const context = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
  const result = await runGetWorkflow(active.workflowId);
  if (result.status !== "ready") {
    finishNonReadyRequest(context);
    useInvestigationStore.getState().detachWorkflow(active.workflowId);
    return result;
  }
  if (
    result.data.workflowId !== active.workflowId ||
    result.data.kind !== active.kind
  ) {
    finishNonReadyRequest(context);
    useInvestigationStore.getState().detachWorkflow(active.workflowId);
    return { status: "incompatible" };
  }

  const data: WorkflowStepResult = {
    workflowId: active.workflowId,
    currentStep: result.data.currentStep,
    stepResult: result.data.stepResult,
    nextExpectedInput: result.data.nextExpectedInput,
  };
  if (!useInvestigationStore.getState().bindWorkflow(context, active.kind, data)) {
    return { status: "stale" };
  }
  return { status: "ready", data };
}

export async function closeWorkspaceWorkflow(): Promise<
  WorkspaceWorkflowResult<CloseWorkflowResult>
> {
  const detached = useInvestigationStore.getState().detachWorkflow();
  if (!detached) {
    return { status: "idle" };
  }
  return runCloseWorkflow(detached.workflowId);
}
