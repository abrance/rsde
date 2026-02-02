import { useState, useEffect } from "react";
import "./ToolPage.css";

interface PromptTemplate {
  id: string;
  name: string;
  description?: string;
  category: string;
  content: string;
  variables: string[];
  tags: string[];
  version: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
  created_by?: string;
}

interface PaginatedResult {
  items: PromptTemplate[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

const CATEGORIES = [
  { value: "chat", label: "Chat" },
  { value: "completion", label: "Completion" },
  { value: "assistant", label: "Assistant" },
  { value: "agent", label: "Agent" },
  { value: "custom", label: "Custom" },
];

export default function PromptPage() {
  const [activeTab, setActiveTab] = useState<
    "overview" | "create" | "list" | "view"
  >("overview");

  // Create form state
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [content, setContent] = useState("");
  const [category, setCategory] = useState("chat");
  const [variables, setVariables] = useState("");
  const [tags, setTags] = useState("");
  const [createdBy, setCreatedBy] = useState("");
  const [creating, setCreating] = useState(false);
  const [createResult, setCreateResult] = useState("");
  const [createdId, setCreatedId] = useState("");

  // List state
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize] = useState(10);
  const [totalPages, setTotalPages] = useState(1);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [searchName, setSearchName] = useState("");

  // View/Edit state
  const [viewId, setViewId] = useState("");
  const [viewedTemplate, setViewedTemplate] = useState<PromptTemplate | null>(
    null,
  );
  const [viewing, setViewing] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editForm, setEditForm] = useState<{
    name: string;
    description: string;
    content: string;
    category: string;
    variables: string;
    tags: string;
    is_active: boolean;
  } | null>(null);

  // Apply template state
  const [variableValues, setVariableValues] = useState<Record<string, string>>(
    {},
  );
  const [appliedContent, setAppliedContent] = useState<string | null>(null);
  const [copySuccess, setCopySuccess] = useState(false);

  useEffect(() => {
    if (activeTab === "list") {
      fetchTemplates();
    }
  }, [activeTab, currentPage]);

  const fetchTemplates = async () => {
    setLoading(true);
    try {
      let url = `/api/prompt/template?page=${currentPage}&page_size=${pageSize}`;
      if (searchName.trim()) {
        url += `&name=${encodeURIComponent(searchName.trim())}`;
      }
      const response = await fetch(url);
      const data = await response.json();
      if (data.success && data.data) {
        const result: PaginatedResult = data.data;
        setTemplates(result.items);
        setTotal(result.total);
        setTotalPages(result.total_pages);
      }
    } catch (error) {
      console.error("Failed to fetch templates:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = () => {
    setCurrentPage(1);
    fetchTemplates();
  };

  const handleCreate = async () => {
    if (!name.trim() || !content.trim()) {
      alert("Name and content are required");
      return;
    }

    setCreating(true);
    setCreateResult("");
    setCreatedId("");

    try {
      const requestBody: Record<string, unknown> = {
        name: name.trim(),
        content: content.trim(),
        category,
      };

      if (description.trim()) requestBody.description = description.trim();
      if (variables.trim())
        requestBody.variables = variables
          .split(",")
          .map((v) => v.trim())
          .filter((v) => v);
      if (tags.trim())
        requestBody.tags = tags
          .split(",")
          .map((t) => t.trim())
          .filter((t) => t);
      if (createdBy.trim()) requestBody.created_by = createdBy.trim();

      const response = await fetch("/api/prompt/template", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(requestBody),
      });

      const data = await response.json();

      if (data.success && data.data) {
        setCreatedId(data.data.id);
        setCreateResult(
          `Created successfully!\n\nID: ${data.data.id}\nName: ${data.data.name}\nVersion: ${data.data.version}\nCreated: ${formatDateTime(data.data.created_at)}`,
        );
        // Clear form
        setContent("");
        setName("");
        setDescription("");
        setVariables("");
        setTags("");
      } else {
        setCreateResult(`Failed: ${data.error || "Unknown error"}`);
      }
    } catch (error) {
      setCreateResult(`Request failed: ${error}`);
    } finally {
      setCreating(false);
    }
  };

  const handleView = async (id?: string) => {
    const targetId = id || viewId;
    if (!targetId.trim()) {
      alert("Please enter a template ID");
      return;
    }

    setViewing(true);
    setViewedTemplate(null);
    setIsEditing(false);
    setEditForm(null);
    resetApplyState();

    try {
      const response = await fetch(`/api/prompt/template/${targetId.trim()}`);
      const data = await response.json();

      if (data.success && data.data) {
        setViewedTemplate(data.data);
      } else {
        alert(`Failed: ${data.error || "Unknown error"}`);
      }
    } catch (error) {
      alert(`Request failed: ${error}`);
    } finally {
      setViewing(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this template?")) {
      return;
    }

    try {
      const response = await fetch(`/api/prompt/template/${id}`, {
        method: "DELETE",
      });
      const data = await response.json();

      if (data.success) {
        alert("Deleted successfully");
        if (activeTab === "list") {
          fetchTemplates();
        } else if (activeTab === "view" && viewedTemplate?.id === id) {
          setViewedTemplate(null);
        }
      } else {
        alert(`Failed: ${data.error || "Unknown error"}`);
      }
    } catch (error) {
      alert(`Request failed: ${error}`);
    }
  };

  const startEdit = () => {
    if (!viewedTemplate) return;
    setEditForm({
      name: viewedTemplate.name,
      description: viewedTemplate.description || "",
      content: viewedTemplate.content,
      category: viewedTemplate.category,
      variables: viewedTemplate.variables.join(", "),
      tags: viewedTemplate.tags.join(", "),
      is_active: viewedTemplate.is_active,
    });
    setIsEditing(true);
  };

  const cancelEdit = () => {
    setIsEditing(false);
    setEditForm(null);
  };

  const handleUpdate = async () => {
    if (!viewedTemplate || !editForm) return;
    if (!editForm.name.trim() || !editForm.content.trim()) {
      alert("Name and content are required");
      return;
    }

    try {
      const requestBody = {
        name: editForm.name.trim(),
        content: editForm.content.trim(),
        description: editForm.description.trim() || null,
        category: editForm.category,
        variables: editForm.variables
          .split(",")
          .map((v) => v.trim())
          .filter((v) => v),
        tags: editForm.tags
          .split(",")
          .map((t) => t.trim())
          .filter((t) => t),
        is_active: editForm.is_active,
      };

      const response = await fetch(
        `/api/prompt/template/${viewedTemplate.id}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(requestBody),
        },
      );

      const data = await response.json();

      if (data.success && data.data) {
        setViewedTemplate(data.data);
        setIsEditing(false);
        setEditForm(null);
        alert("Updated successfully");
      } else {
        alert(`Failed: ${data.error || "Unknown error"}`);
      }
    } catch (error) {
      alert(`Request failed: ${error}`);
    }
  };

  const formatDateTime = (dateStr: string) => {
    return new Date(dateStr).toLocaleString("zh-CN");
  };

  const getCategoryLabel = (value: string) => {
    return CATEGORIES.find((c) => c.value === value)?.label || value;
  };

  const extractVariables = (content: string): string[] => {
    const regex = /\{\{(\w+)\}\}/g;
    const variables = new Set<string>();
    let match;
    while ((match = regex.exec(content)) !== null) {
      variables.add(match[1]);
    }
    return Array.from(variables);
  };

  const applyTemplate = () => {
    if (!viewedTemplate) return;
    let result = viewedTemplate.content;
    const vars = extractVariables(result);
    for (const v of vars) {
      const value = variableValues[v] || "";
      result = result.replace(new RegExp(`\\{\\{${v}\\}\\}`, "g"), value);
    }
    setAppliedContent(result);
    setCopySuccess(false);
  };

  const copyToClipboard = async () => {
    if (!appliedContent) return;
    try {
      await navigator.clipboard.writeText(appliedContent);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch {
      alert("Failed to copy");
    }
  };

  const resetApplyState = () => {
    setVariableValues({});
    setAppliedContent(null);
    setCopySuccess(false);
  };

  return (
    <div className="tool-page">
      <div className="page-header">
        <h1 className="page-title">
          <span className="page-icon">💬</span>
          Prompt - Template Management
        </h1>
        <p className="page-description">
          Manage prompt templates for AI applications with versioning and
          categorization
        </p>
      </div>

      <div className="tabs">
        <button
          className={`tab ${activeTab === "overview" ? "active" : ""}`}
          onClick={() => setActiveTab("overview")}
        >
          Overview
        </button>
        <button
          className={`tab ${activeTab === "create" ? "active" : ""}`}
          onClick={() => setActiveTab("create")}
        >
          Create
        </button>
        <button
          className={`tab ${activeTab === "list" ? "active" : ""}`}
          onClick={() => setActiveTab("list")}
        >
          Templates
        </button>
        <button
          className={`tab ${activeTab === "view" ? "active" : ""}`}
          onClick={() => setActiveTab("view")}
        >
          View
        </button>
      </div>

      <div className="tab-content">
        {activeTab === "overview" && (
          <div className="overview">
            <div className="card">
              <h2>Features</h2>
              <ul className="feature-list">
                <li>📝 Create and manage prompt templates</li>
                <li>
                  📂 Categorize prompts (Chat, Completion, Assistant, Agent,
                  Custom)
                </li>
                <li>🔢 Automatic version control</li>
                <li>🏷️ Tag-based organization</li>
                <li>📋 Variable placeholder support</li>
                <li>🔄 Active/Inactive status management</li>
                <li>🔍 Search by name</li>
              </ul>
            </div>

            <div className="card">
              <h2>Categories</h2>
              <div className="format-grid">
                {CATEGORIES.map((cat) => (
                  <span key={cat.value} className="format-badge">
                    {cat.label}
                  </span>
                ))}
              </div>
            </div>

            <div className="card">
              <h2>Usage</h2>
              <ol style={{ lineHeight: "1.8" }}>
                <li>Create a new template in the "Create" tab</li>
                <li>
                  Define variables using placeholders like {"{{variable_name}}"}
                </li>
                <li>Organize with categories and tags</li>
                <li>Browse and search templates in the "Templates" tab</li>
                <li>View, edit, or delete templates as needed</li>
              </ol>
            </div>
          </div>
        )}

        {activeTab === "create" && (
          <div className="create-panel">
            <div className="card">
              <h2>Create New Template</h2>

              <div className="form-group">
                <label htmlFor="name">Name *</label>
                <input
                  id="name"
                  type="text"
                  className="input"
                  placeholder="Enter template name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label htmlFor="description">Description (optional)</label>
                <input
                  id="description"
                  type="text"
                  className="input"
                  placeholder="Brief description of the template"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label htmlFor="content">Content *</label>
                <textarea
                  id="content"
                  className="input"
                  placeholder="Enter prompt content. Use {{variable}} for placeholders..."
                  value={content}
                  onChange={(e) => setContent(e.target.value)}
                  rows={10}
                  style={{ fontFamily: "monospace" }}
                />
              </div>

              <div className="form-group">
                <label htmlFor="category">Category</label>
                <select
                  id="category"
                  className="input"
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                >
                  {CATEGORIES.map((cat) => (
                    <option key={cat.value} value={cat.value}>
                      {cat.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label htmlFor="variables">Variables (comma-separated)</label>
                <input
                  id="variables"
                  type="text"
                  className="input"
                  placeholder="e.g., user_name, topic, language"
                  value={variables}
                  onChange={(e) => setVariables(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label htmlFor="tags">Tags (comma-separated)</label>
                <input
                  id="tags"
                  type="text"
                  className="input"
                  placeholder="e.g., coding, translation, summary"
                  value={tags}
                  onChange={(e) => setTags(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label htmlFor="createdBy">Created By (optional)</label>
                <input
                  id="createdBy"
                  type="text"
                  className="input"
                  placeholder="Your name or identifier"
                  value={createdBy}
                  onChange={(e) => setCreatedBy(e.target.value)}
                />
              </div>

              <button
                className="btn"
                onClick={handleCreate}
                disabled={creating}
              >
                {creating ? "Creating..." : "💬 Create Template"}
              </button>

              {createResult && (
                <div className="result-box">
                  <pre className="result-content">{createResult}</pre>
                  {createdId && (
                    <button
                      className="btn"
                      onClick={() => {
                        setViewId(createdId);
                        setActiveTab("view");
                        setTimeout(() => handleView(createdId), 100);
                      }}
                      style={{ marginTop: "10px" }}
                    >
                      👁️ View Template
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>
        )}

        {activeTab === "list" && (
          <div className="list-panel">
            <div className="card">
              <h2>Template List</h2>

              <div className="form-group" style={{ marginBottom: "20px" }}>
                <div style={{ display: "flex", gap: "10px" }}>
                  <input
                    type="text"
                    className="input"
                    placeholder="Search by name..."
                    value={searchName}
                    onChange={(e) => setSearchName(e.target.value)}
                    onKeyPress={(e) => e.key === "Enter" && handleSearch()}
                    style={{ flex: 1 }}
                  />
                  <button className="btn" onClick={handleSearch}>
                    🔍 Search
                  </button>
                  <button
                    className="btn"
                    onClick={() => {
                      setSearchName("");
                      fetchTemplates();
                    }}
                    style={{ backgroundColor: "#6c757d" }}
                  >
                    Clear
                  </button>
                </div>
              </div>

              <p style={{ marginBottom: "20px" }}>
                Total: {total} templates | Page {currentPage}/{totalPages}
              </p>

              {loading ? (
                <p className="loading-text">Loading...</p>
              ) : templates.length === 0 ? (
                <p className="placeholder-text">No templates found</p>
              ) : (
                <>
                  <div className="template-list">
                    {templates.map((template) => (
                      <div
                        key={template.id}
                        className="template-item card"
                        style={{ marginBottom: "15px", padding: "15px" }}
                      >
                        <div
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "start",
                          }}
                        >
                          <div style={{ flex: 1 }}>
                            <h3
                              style={{
                                margin: "0 0 10px 0",
                                display: "flex",
                                alignItems: "center",
                                gap: "10px",
                              }}
                            >
                              {template.name}
                              <span
                                className="format-badge"
                                style={{ fontSize: "0.8em" }}
                              >
                                {getCategoryLabel(template.category)}
                              </span>
                              {!template.is_active && (
                                <span
                                  style={{
                                    fontSize: "0.8em",
                                    color: "#dc3545",
                                  }}
                                >
                                  (Inactive)
                                </span>
                              )}
                            </h3>
                            {template.description && (
                              <p
                                style={{ margin: "0 0 10px 0", color: "#666" }}
                              >
                                {template.description}
                              </p>
                            )}
                            <div
                              style={{
                                fontSize: "0.9em",
                                color: "#666",
                                marginBottom: "10px",
                              }}
                            >
                              <span>📋 v{template.version}</span>
                              <span style={{ margin: "0 10px" }}>|</span>
                              <span>
                                🕒 {formatDateTime(template.updated_at)}
                              </span>
                              {template.created_by && (
                                <>
                                  <span style={{ margin: "0 10px" }}>|</span>
                                  <span>👤 {template.created_by}</span>
                                </>
                              )}
                            </div>
                            {template.tags.length > 0 && (
                              <div style={{ marginBottom: "10px" }}>
                                {template.tags.map((tag, i) => (
                                  <span
                                    key={i}
                                    className="format-badge"
                                    style={{
                                      marginRight: "5px",
                                      backgroundColor: "#6c757d",
                                    }}
                                  >
                                    {tag}
                                  </span>
                                ))}
                              </div>
                            )}
                            <div
                              style={{
                                maxHeight: "80px",
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                backgroundColor: "#f5f5f5",
                                padding: "10px",
                                borderRadius: "4px",
                                fontFamily: "monospace",
                                fontSize: "0.85em",
                              }}
                            >
                              {template.content.substring(0, 200)}
                              {template.content.length > 200 && "..."}
                            </div>
                          </div>
                          <div
                            style={{
                              marginLeft: "15px",
                              display: "flex",
                              flexDirection: "column",
                              gap: "5px",
                            }}
                          >
                            <button
                              className="btn"
                              onClick={() => {
                                setViewId(template.id);
                                setActiveTab("view");
                                setTimeout(() => handleView(template.id), 100);
                              }}
                              style={{ fontSize: "0.9em", padding: "5px 10px" }}
                            >
                              View
                            </button>
                            <button
                              className="btn"
                              onClick={() => handleDelete(template.id)}
                              style={{
                                fontSize: "0.9em",
                                padding: "5px 10px",
                                backgroundColor: "#dc3545",
                              }}
                            >
                              Delete
                            </button>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>

                  <div
                    style={{
                      display: "flex",
                      justifyContent: "center",
                      gap: "10px",
                      marginTop: "20px",
                    }}
                  >
                    <button
                      className="btn"
                      onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                      disabled={currentPage === 1}
                    >
                      Previous
                    </button>
                    <span style={{ lineHeight: "40px" }}>
                      {currentPage} / {totalPages}
                    </span>
                    <button
                      className="btn"
                      onClick={() =>
                        setCurrentPage((p) => Math.min(totalPages, p + 1))
                      }
                      disabled={currentPage === totalPages}
                    >
                      Next
                    </button>
                  </div>
                </>
              )}
            </div>
          </div>
        )}

        {activeTab === "view" && (
          <div className="view-panel">
            <div className="card">
              <h2>View Template</h2>

              <div className="form-group">
                <label htmlFor="viewId">Template ID</label>
                <div style={{ display: "flex", gap: "10px" }}>
                  <input
                    id="viewId"
                    type="text"
                    className="input"
                    placeholder="Enter template ID"
                    value={viewId}
                    onChange={(e) => setViewId(e.target.value)}
                    style={{ flex: 1 }}
                  />
                  <button
                    className="btn"
                    onClick={() => handleView()}
                    disabled={viewing}
                  >
                    {viewing ? "Loading..." : "👁️ View"}
                  </button>
                </div>
              </div>

              {viewedTemplate && !isEditing && (
                <div className="result-box">
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "start",
                      marginBottom: "15px",
                    }}
                  >
                    <h3 style={{ margin: 0 }}>{viewedTemplate.name}</h3>
                    <div style={{ display: "flex", gap: "10px" }}>
                      <button
                        className="btn"
                        onClick={startEdit}
                        style={{ padding: "5px 15px" }}
                      >
                        ✏️ Edit
                      </button>
                      <button
                        className="btn"
                        onClick={() => handleDelete(viewedTemplate.id)}
                        style={{
                          padding: "5px 15px",
                          backgroundColor: "#dc3545",
                        }}
                      >
                        🗑️ Delete
                      </button>
                    </div>
                  </div>
                  <div
                    style={{
                      fontSize: "0.9em",
                      color: "#666",
                      marginBottom: "15px",
                    }}
                  >
                    <p>🆔 ID: {viewedTemplate.id}</p>
                    <p>
                      📂 Category: {getCategoryLabel(viewedTemplate.category)}
                    </p>
                    {viewedTemplate.description && (
                      <p>📝 Description: {viewedTemplate.description}</p>
                    )}
                    <p>📋 Version: {viewedTemplate.version}</p>
                    <p>
                      ✅ Status:{" "}
                      {viewedTemplate.is_active ? "Active" : "Inactive"}
                    </p>
                    {viewedTemplate.created_by && (
                      <p>👤 Created by: {viewedTemplate.created_by}</p>
                    )}
                    <p>
                      🕒 Created: {formatDateTime(viewedTemplate.created_at)}
                    </p>
                    <p>
                      🔄 Updated: {formatDateTime(viewedTemplate.updated_at)}
                    </p>
                    {viewedTemplate.variables.length > 0 && (
                      <div style={{ marginTop: "10px" }}>
                        <span>📌 Variables: </span>
                        {viewedTemplate.variables.map((v, i) => (
                          <span
                            key={i}
                            className="format-badge"
                            style={{
                              marginRight: "5px",
                              backgroundColor: "#17a2b8",
                            }}
                          >
                            {v}
                          </span>
                        ))}
                      </div>
                    )}
                    {viewedTemplate.tags.length > 0 && (
                      <div style={{ marginTop: "10px" }}>
                        <span>🏷️ Tags: </span>
                        {viewedTemplate.tags.map((tag, i) => (
                          <span
                            key={i}
                            className="format-badge"
                            style={{
                              marginRight: "5px",
                              backgroundColor: "#6c757d",
                            }}
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                  <div style={{ marginTop: "20px" }}>
                    <h4>Content:</h4>
                    <pre
                      className="result-content"
                      style={{
                        whiteSpace: "pre-wrap",
                        wordBreak: "break-word",
                      }}
                    >
                      {viewedTemplate.content}
                    </pre>
                  </div>

                  <div
                    style={{
                      marginTop: "30px",
                      borderTop: "1px solid var(--border-color)",
                      paddingTop: "20px",
                    }}
                  >
                    <h4>🚀 Apply Template</h4>
                    {(() => {
                      const vars = extractVariables(viewedTemplate.content);
                      if (vars.length === 0) {
                        return (
                          <div style={{ marginTop: "15px" }}>
                            <p style={{ color: "#666", marginBottom: "15px" }}>
                              No variables found in this template.
                            </p>
                            <button className="btn" onClick={applyTemplate}>
                              📋 Apply Template
                            </button>
                          </div>
                        );
                      }
                      return (
                        <div style={{ marginTop: "15px" }}>
                          <p style={{ color: "#666", marginBottom: "15px" }}>
                            Found {vars.length} variable(s). Fill in the values
                            below:
                          </p>
                          <table
                            style={{
                              width: "100%",
                              borderCollapse: "collapse",
                              marginBottom: "15px",
                            }}
                          >
                            <thead>
                              <tr style={{ backgroundColor: "var(--card-bg)" }}>
                                <th
                                  style={{
                                    padding: "10px",
                                    textAlign: "left",
                                    borderBottom:
                                      "1px solid var(--border-color)",
                                    width: "30%",
                                  }}
                                >
                                  Variable
                                </th>
                                <th
                                  style={{
                                    padding: "10px",
                                    textAlign: "left",
                                    borderBottom:
                                      "1px solid var(--border-color)",
                                  }}
                                >
                                  Value
                                </th>
                              </tr>
                            </thead>
                            <tbody>
                              {vars.map((v) => (
                                <tr key={v}>
                                  <td
                                    style={{
                                      padding: "10px",
                                      borderBottom:
                                        "1px solid var(--border-color)",
                                    }}
                                  >
                                    <code
                                      style={{
                                        backgroundColor: "#f5f5f5",
                                        padding: "2px 6px",
                                        borderRadius: "4px",
                                      }}
                                    >
                                      {`{{${v}}}`}
                                    </code>
                                  </td>
                                  <td
                                    style={{
                                      padding: "10px",
                                      borderBottom:
                                        "1px solid var(--border-color)",
                                    }}
                                  >
                                    <textarea
                                      className="input"
                                      placeholder={`Enter value for ${v}`}
                                      value={variableValues[v] || ""}
                                      onChange={(e) =>
                                        setVariableValues({
                                          ...variableValues,
                                          [v]: e.target.value,
                                        })
                                      }
                                      rows={3}
                                      style={{
                                        margin: 0,
                                        fontFamily: "monospace",
                                        resize: "vertical",
                                        minHeight: "60px",
                                      }}
                                    />
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                          <button className="btn" onClick={applyTemplate}>
                            🚀 Apply Template
                          </button>
                        </div>
                      );
                    })()}

                    {appliedContent !== null && (
                      <div style={{ marginTop: "20px" }}>
                        <div
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "center",
                            marginBottom: "10px",
                          }}
                        >
                          <h4 style={{ margin: 0 }}>Result:</h4>
                          <button
                            className="btn"
                            onClick={copyToClipboard}
                            style={{
                              padding: "5px 15px",
                              backgroundColor: copySuccess
                                ? "#28a745"
                                : undefined,
                            }}
                          >
                            {copySuccess
                              ? "✅ Copied!"
                              : "📋 Copy to Clipboard"}
                          </button>
                        </div>
                        <pre
                          className="result-content"
                          style={{
                            whiteSpace: "pre-wrap",
                            wordBreak: "break-word",
                            backgroundColor: "#1a1a2e",
                            border: "2px solid #28a745",
                          }}
                        >
                          {appliedContent}
                        </pre>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {viewedTemplate && isEditing && editForm && (
                <div className="result-box">
                  <h3 style={{ marginBottom: "20px" }}>Edit Template</h3>

                  <div className="form-group">
                    <label>Name *</label>
                    <input
                      type="text"
                      className="input"
                      value={editForm.name}
                      onChange={(e) =>
                        setEditForm({ ...editForm, name: e.target.value })
                      }
                    />
                  </div>

                  <div className="form-group">
                    <label>Description</label>
                    <input
                      type="text"
                      className="input"
                      value={editForm.description}
                      onChange={(e) =>
                        setEditForm({
                          ...editForm,
                          description: e.target.value,
                        })
                      }
                    />
                  </div>

                  <div className="form-group">
                    <label>Content *</label>
                    <textarea
                      className="input"
                      value={editForm.content}
                      onChange={(e) =>
                        setEditForm({ ...editForm, content: e.target.value })
                      }
                      rows={10}
                      style={{ fontFamily: "monospace" }}
                    />
                  </div>

                  <div className="form-group">
                    <label>Category</label>
                    <select
                      className="input"
                      value={editForm.category}
                      onChange={(e) =>
                        setEditForm({ ...editForm, category: e.target.value })
                      }
                    >
                      {CATEGORIES.map((cat) => (
                        <option key={cat.value} value={cat.value}>
                          {cat.label}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div className="form-group">
                    <label>Variables (comma-separated)</label>
                    <input
                      type="text"
                      className="input"
                      value={editForm.variables}
                      onChange={(e) =>
                        setEditForm({ ...editForm, variables: e.target.value })
                      }
                    />
                  </div>

                  <div className="form-group">
                    <label>Tags (comma-separated)</label>
                    <input
                      type="text"
                      className="input"
                      value={editForm.tags}
                      onChange={(e) =>
                        setEditForm({ ...editForm, tags: e.target.value })
                      }
                    />
                  </div>

                  <div className="form-group">
                    <label className="checkbox-label">
                      <input
                        type="checkbox"
                        checked={editForm.is_active}
                        onChange={(e) =>
                          setEditForm({
                            ...editForm,
                            is_active: e.target.checked,
                          })
                        }
                      />
                      Active
                    </label>
                  </div>

                  <div style={{ display: "flex", gap: "10px" }}>
                    <button className="btn" onClick={handleUpdate}>
                      💾 Save Changes
                    </button>
                    <button
                      className="btn"
                      onClick={cancelEdit}
                      style={{ backgroundColor: "#6c757d" }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
