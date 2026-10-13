declare module "markdown-it-task-lists" {
  import type { PluginWithOptions } from "markdown-it";

  type TaskListsOptions = {
    enabled?: boolean;
    label?: boolean;
    labelAfter?: boolean;
  };

  const taskLists: PluginWithOptions<TaskListsOptions>;
  export default taskLists;
}
