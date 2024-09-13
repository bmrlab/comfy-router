import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import Index from "./pages";

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Index />
    </QueryClientProvider>
  );
}

export default App;
