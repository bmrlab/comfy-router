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
import { Input } from "@/components/ui/input";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

type NodeStatus = {
  url: string;
  status: {
    status: "busy" | "idle";
    cache: object;
  };
};

export default function Index() {
  const { data, refetch } = useQuery({
    queryKey: ["nodes"],
    queryFn: async () => {
      const response = await fetch(
        import.meta.env.DEV
          ? "http://localhost:8080/cluster/nodes"
          : "/cluster/nodes"
      );
      const data = (await response.json()) as { nodes: NodeStatus[] };
      return data;
    },
    refetchInterval: 1000,
  });

  const { mutateAsync } = useMutation({
    mutationKey: ["add", "node"],
    mutationFn: async (url: string) => {
      await fetch(
        import.meta.env.DEV
          ? "http://localhost:8080/cluster/join"
          : "/cluster/join",
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            url,
          }),
        }
      );
    },
  });

  const form = useForm({
    defaultValues: {
      url: "",
    },
    onSubmit: async ({ value }) => {
      await mutateAsync(value.url);
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
      <div className="pt-8">
        {data?.nodes.map((node) => (
          <div
            key={node.url}
            className="py-4 px-4 flex flex-col w-full border rounded-md border-gray-300"
          >
            <div className="">{node.url}</div>
            <div className="flex items-center justify-start mt-4">
              <Badge
                className={
                  node.status.status === "idle"
                    ? "bg-green-600"
                    : "bg-yellow-600"
                }
              >
                {node.status.status}
              </Badge>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
