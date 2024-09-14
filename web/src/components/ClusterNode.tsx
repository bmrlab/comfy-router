import type { NodeStatus } from "@/pages";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import api from "@/lib/api";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { Input } from "./ui/input";

function ClusterNode({
  url,
  status,
  refetch,
}: NodeStatus & { refetch: () => Promise<void> }) {
  const { mutateAsync: removeNode } = useMutation({
    mutationKey: ["remove", "node"],
    mutationFn: async (url: string) => {
      await api.post("/cluster/nodes/delete", { url });
    },
  });

  const form = useForm({
    defaultValues: {
      url: "",
    },
    onSubmit: async () => {
      await removeNode(url);
      await refetch();
      setOpen(false);
      form.reset();
    },
  });

  const [open, setOpen] = useState(false);

  return (
    <div className="py-4 px-4 w-full border rounded-md border-gray-300 flex justify-between">
      <div className="flex flex-col">
        <div className="">{url}</div>
        <div className="flex items-center justify-start mt-4">
          <Badge
            className={
              status.status === "idle"
                ? "bg-green-600"
                : status.status === "busy"
                ? "bg-yellow-600"
                : "bg-red-600"
            }
          >
            {status.status}
          </Badge>
        </div>
      </div>

      <div className="flex items-center justify-end">
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild>
            <Button className="" variant="destructive">
              Remove
            </Button>
          </DialogTrigger>
          <DialogContent className="sm:max-w-[425px]">
            <form
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();
                form.handleSubmit();
              }}
            >
              <DialogHeader>
                <DialogTitle>Remove node</DialogTitle>
                <DialogDescription>{`Are you sure to remove ${url}`}</DialogDescription>
              </DialogHeader>

              <div className="grid gap-4 py-4">
                <div className="grid grid-cols-4 items-center gap-4">
                  <form.Field
                    name="url"
                    validators={{
                      onChange: ({ value }) =>
                        value === url ? void 0 : "The url is not the same.",
                    }}
                  >
                    {(field) => (
                      <Input
                        id={field.name}
                        name={field.name}
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className="col-span-4"
                        placeholder="Please type the node url"
                      />
                    )}
                  </form.Field>
                </div>
              </div>

              <DialogFooter>
                <form.Subscribe
                  selector={(state) => [
                    state.canSubmit,
                    state.isTouched,
                    state.isSubmitting,
                  ]}
                >
                  {([canSubmit, isTouched, isSubmitting]) => (
                    <Button type="submit" disabled={!canSubmit || !isTouched}>
                      {isSubmitting ? "Removing..." : "Remove"}
                    </Button>
                  )}
                </form.Subscribe>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
}

export default ClusterNode;
