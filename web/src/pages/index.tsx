import ClusterNode from "@/components/ClusterNode";
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
import { Input } from "@/components/ui/input";
import api from "@/lib/api";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

export type NodeStatus = {
  url: string;
  status: {
    status: "busy" | "idle" | "offline";
    cache: object;
  };
};

export default function Index() {
  const { data, refetch } = useQuery({
    queryKey: ["nodes"],
    queryFn: async () => {
      const response = await api.get<{ nodes: NodeStatus[] }>("/cluster/nodes");
      return response.data;
    },
    refetchInterval: 1000,
  });

  const { mutateAsync: addNode } = useMutation({
    mutationKey: ["add", "node"],
    mutationFn: async (url: string) => {
      await api.post("/cluster/nodes", { url });
    },
  });

  const form = useForm({
    defaultValues: {
      url: "",
    },
    onSubmit: async ({ value }) => {
      await addNode(value.url);
      await refetch();
      setOpen(false);
      form.reset();
    },
  });

  const [open, setOpen] = useState(false);

  return (
    <div className="mx-auto w-full px-8 max-w-screen-xl">
      <div className="flex justify-between items-center pt-24">
        <div className="font-bold text-xl">ComfyUI Nodes List</div>

        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild>
            <Button variant="outline">Add node</Button>
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
                <DialogTitle>Add node</DialogTitle>
                <DialogDescription>Add a new ComfyUI node.</DialogDescription>
              </DialogHeader>
              <div className="grid gap-4 py-4">
                <div className="grid grid-cols-4 items-center gap-4">
                  <form.Field name="url">
                    {(field) => (
                      <Input
                        id={field.name}
                        name={field.name}
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className="col-span-4"
                      />
                    )}
                  </form.Field>
                </div>
              </div>
              <DialogFooter>
                <form.Subscribe
                  selector={(state) => [state.canSubmit, state.isSubmitting]}
                >
                  {([canSubmit, isSubmitting]) => (
                    <Button type="submit" disabled={!canSubmit}>
                      {isSubmitting ? "Submitting..." : "Submit"}
                    </Button>
                  )}
                </form.Subscribe>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>

      {/* list content */}
      <div className="pt-8 flex flex-col space-y-8">
        {data?.nodes.map((node) => (
          <ClusterNode
            key={node.url}
            {...node}
            refetch={async () => {
              await refetch();
            }}
          />
        ))}
      </div>
    </div>
  );
}
