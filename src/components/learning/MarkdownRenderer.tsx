import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import { CodeBlock } from "./CodeBlock";
import type { Components } from "react-markdown";

interface MarkdownRendererProps {
  content: string;
  className?: string;
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}

function handleExternalLink(e: React.MouseEvent<HTMLAnchorElement>) {
  const href = e.currentTarget.getAttribute("href");
  if (href && (href.startsWith("http://") || href.startsWith("https://"))) {
    e.preventDefault();
    // Use Tauri shell open for external links
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open(href);
    }).catch(() => {
      window.open(href, "_blank");
    });
  }
}

const components: Components = {
  h1: ({ children, ...props }) => {
    const text = String(children);
    return (
      <h1
        id={slugify(text)}
        className="mb-4 mt-8 scroll-mt-20 text-3xl font-bold text-foreground first:mt-0"
        {...props}
      >
        {children}
      </h1>
    );
  },
  h2: ({ children, ...props }) => {
    const text = String(children);
    return (
      <h2
        id={slugify(text)}
        className="mb-3 mt-8 scroll-mt-20 border-b border-border pb-2 text-2xl font-semibold text-foreground"
        {...props}
      >
        {children}
      </h2>
    );
  },
  h3: ({ children, ...props }) => {
    const text = String(children);
    return (
      <h3
        id={slugify(text)}
        className="mb-2 mt-6 scroll-mt-20 text-xl font-semibold text-foreground"
        {...props}
      >
        {children}
      </h3>
    );
  },
  h4: ({ children, ...props }) => {
    const text = String(children);
    return (
      <h4
        id={slugify(text)}
        className="mb-2 mt-4 scroll-mt-20 text-lg font-medium text-foreground"
        {...props}
      >
        {children}
      </h4>
    );
  },
  p: ({ children, ...props }) => (
    <p className="mb-4 leading-7 text-foreground/90" {...props}>
      {children}
    </p>
  ),
  a: ({ children, href, ...props }) => (
    <a
      href={href}
      onClick={handleExternalLink}
      className="font-medium text-primary underline underline-offset-4 hover:text-primary/80"
      {...props}
    >
      {children}
    </a>
  ),
  ul: ({ children, ...props }) => (
    <ul className="mb-4 ml-6 list-disc space-y-1 text-foreground/90" {...props}>
      {children}
    </ul>
  ),
  ol: ({ children, ...props }) => (
    <ol className="mb-4 ml-6 list-decimal space-y-1 text-foreground/90" {...props}>
      {children}
    </ol>
  ),
  li: ({ children, ...props }) => (
    <li className="leading-7" {...props}>
      {children}
    </li>
  ),
  blockquote: ({ children, ...props }) => (
    <blockquote
      className="mb-4 border-l-4 border-primary/50 bg-secondary/30 py-2 pl-4 pr-3 italic text-muted-foreground"
      {...props}
    >
      {children}
    </blockquote>
  ),
  table: ({ children, ...props }) => (
    <div className="mb-4 overflow-x-auto rounded-lg border border-border">
      <table className="w-full text-sm" {...props}>
        {children}
      </table>
    </div>
  ),
  thead: ({ children, ...props }) => (
    <thead className="border-b border-border bg-secondary/50" {...props}>
      {children}
    </thead>
  ),
  th: ({ children, ...props }) => (
    <th className="px-4 py-2 text-left font-semibold text-foreground" {...props}>
      {children}
    </th>
  ),
  td: ({ children, ...props }) => (
    <td className="border-t border-border px-4 py-2 text-foreground/90" {...props}>
      {children}
    </td>
  ),
  hr: () => <hr className="my-6 border-border" />,
  code: ({ children, className, ...props }) => {
    const match = /language-(\w+)/.exec(className || "");
    const codeString = String(children).replace(/\n$/, "");

    if (match) {
      return <CodeBlock code={codeString} language={match[1]} />;
    }

    return (
      <code
        className="rounded-md bg-secondary/80 px-1.5 py-0.5 font-mono text-sm text-primary"
        {...props}
      >
        {children}
      </code>
    );
  },
  pre: ({ children }) => {
    // The code component handles fenced code blocks, so pre just passes through
    return <>{children}</>;
  },
  img: ({ src, alt, ...props }) => (
    <img
      src={src}
      alt={alt}
      className="my-4 max-w-full rounded-lg border border-border"
      loading="lazy"
      {...props}
    />
  ),
  strong: ({ children, ...props }) => (
    <strong className="font-semibold text-foreground" {...props}>
      {children}
    </strong>
  ),
  em: ({ children, ...props }) => (
    <em className="italic text-foreground/80" {...props}>
      {children}
    </em>
  ),
};

export function MarkdownRenderer({ content, className }: MarkdownRendererProps) {
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw]}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
