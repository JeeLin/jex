//! 项目模板（jex create）
//! - create: 根据模板创建新项目

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 模板信息
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
}

/// 获取所有可用模板
pub fn list_templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo {
            name: "lib".to_string(),
            description: "基础 Java 库".to_string(),
        },
        TemplateInfo {
            name: "cli".to_string(),
            description: "命令行工具".to_string(),
        },
        TemplateInfo {
            name: "api".to_string(),
            description: "REST API 服务".to_string(),
        },
        TemplateInfo {
            name: "web".to_string(),
            description: "Web 应用".to_string(),
        },
    ]
}

/// 模板内容
struct TemplateContent {
    jex_toml: &'static str,
    main_java: &'static str,
}

/// 获取模板内容
fn get_template_content(template: &str) -> Result<TemplateContent> {
    match template {
        "lib" => Ok(TemplateContent {
            jex_toml: r#"# {{project_name}} - Java Library

[project]
name = "{{project_name}}"
"#,
            main_java: r#"package {{package_name}};

/**
 * {{project_name}} - Java Library
 */
public class Main {
    public static void main(String[] args) {
        System.out.println("Hello from {{project_name}}!");
    }
}
"#,
        }),
        "cli" => Ok(TemplateContent {
            jex_toml: r#"# {{project_name}} - CLI Tool

[project]
name = "{{project_name}}"
"#,
            main_java: r#"package {{package_name}};

/**
 * {{project_name}} - CLI Tool
 */
public class Main {
    public static void main(String[] args) {
        if (args.length > 0) {
            System.out.println("Hello, " + args[0] + "!");
        } else {
            System.out.println("Hello from {{project_name}}!");
        }
    }
}
"#,
        }),
        "api" => Ok(TemplateContent {
            jex_toml: r#"# {{project_name}} - REST API

[project]
name = "{{project_name}}"

[dependencies]
"com.sun.net.httpserver:http:1.0"
"#,
            main_java: r#"package {{package_name}};

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.io.OutputStream;

/**
 * {{project_name}} - REST API Server
 */
public class Main {
    public static void main(String[] args) throws IOException {
        HttpServer server = HttpServer.create(new java.net.InetSocketAddress(8080), 0);
        server.createContext("/", new RootHandler());
        server.setExecutor(null);
        server.start();
        System.out.println("Server started on port 8080");
    }

    static class RootHandler implements HttpHandler {
        @Override
        public void handle(HttpExchange exchange) throws IOException {
            String response = "{\"message\": \"Hello from {{project_name}}!\"}";
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            OutputStream os = exchange.getResponseBody();
            os.write(response.getBytes());
            os.close();
        }
    }
}
"#,
        }),
        "web" => Ok(TemplateContent {
            jex_toml: r#"# {{project_name}} - Web Application

[project]
name = "{{project_name}}"
"#,
            main_java: r#"package {{package_name}};

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.io.OutputStream;

/**
 * {{project_name}} - Web Application
 */
public class Main {
    public static void main(String[] args) throws IOException {
        HttpServer server = HttpServer.create(new java.net.InetSocketAddress(8080), 0);
        server.createContext("/", new RootHandler());
        server.setExecutor(null);
        server.start();
        System.out.println("Web app started on http://localhost:8080");
    }

    static class RootHandler implements HttpHandler {
        @Override
        public void handle(HttpExchange exchange) throws IOException {
            String html = "<html><body><h1>Hello from {{project_name}}!</h1></body></html>";
            exchange.getResponseHeaders().set("Content-Type", "text/html");
            exchange.sendResponseHeaders(200, html.length());
            OutputStream os = exchange.getResponseBody();
            os.write(html.getBytes());
            os.close();
        }
    }
}
"#,
        }),
        _ => Err(Error::new(format!("未知模板类型: {}", template))),
    }
}

/// 替换模板变量
fn replace_variables(content: &str, variables: &HashMap<String, &str>) -> String {
    let mut result = content.to_string();
    for (key, value) in variables {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

/// 创建项目
pub fn create_project(name: &str, template: &str, package: Option<&str>) -> Result<()> {
    let templates = list_templates();
    if !templates.iter().any(|t| t.name == template) {
        let available: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        return Err(Error::new(format!(
            "未知模板类型: {}，可用模板: {}",
            template,
            available.join(", ")
        )));
    }

    let template_content = get_template_content(template)?;
    let package_name = package.unwrap_or("com.example");
    let project_dir = Path::new(name);

    if project_dir.exists() {
        return Err(Error::new(format!("目录已存在: {}", name)));
    }

    // 创建目录结构
    fs::create_dir_all(project_dir.join("src"))?;

    // 替换变量
    let mut variables: HashMap<String, &str> = HashMap::new();
    variables.insert("project_name".to_string(), name);
    variables.insert("package_name".to_string(), package_name);

    // 创建 jex.toml
    let jex_toml_content = replace_variables(template_content.jex_toml, &variables);
    fs::write(project_dir.join("jex.toml"), jex_toml_content)?;

    // 创建 Main.java
    let main_java_content = replace_variables(template_content.main_java, &variables);
    let java_path = project_dir.join("src").join("Main.java");
    fs::write(java_path, main_java_content)?;

    println!("✅ 项目 '{}' 已创建", name);
    println!("   模板: {}", template);
    println!("   目录: {}", project_dir.display());
    println!();
    println!("下一步:");
    println!("   cd {}", name);
    println!("   jex run src/Main.java");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let templates = list_templates();
        assert_eq!(templates.len(), 4);
        assert!(templates.iter().any(|t| t.name == "lib"));
        assert!(templates.iter().any(|t| t.name == "cli"));
        assert!(templates.iter().any(|t| t.name == "api"));
        assert!(templates.iter().any(|t| t.name == "web"));
    }

    #[test]
    fn test_replace_variables() {
        let content = "Hello {{project_name}} by {{package_name}}";
        let mut variables: HashMap<String, &str> = HashMap::new();
        variables.insert("project_name".to_string(), "my-app");
        variables.insert("package_name".to_string(), "com.example");

        let result = replace_variables(content, &variables);
        assert_eq!(result, "Hello my-app by com.example");
    }

    #[test]
    fn test_create_project_lib() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("test-lib");
        let project_name = project_dir.to_str().unwrap();

        let result = create_project(project_name, "lib", Some("com.test"));
        assert!(result.is_ok());

        // 检查目录结构
        assert!(project_dir.join("jex.toml").exists());
        assert!(project_dir.join("src").join("Main.java").exists());

        // 检查 jex.toml 内容
        let jex_toml = fs::read_to_string(project_dir.join("jex.toml")).unwrap();
        assert!(jex_toml.contains("test-lib"));

        // 检查 Main.java 内容
        let main_java = fs::read_to_string(project_dir.join("src").join("Main.java")).unwrap();
        assert!(main_java.contains("com.test"));
    }

    #[test]
    fn test_create_project_invalid_template() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("test-invalid");
        let project_name = project_dir.to_str().unwrap();

        let result = create_project(project_name, "invalid", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_project_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("existing");
        fs::create_dir(&project_dir).unwrap();

        let result = create_project(project_dir.to_str().unwrap(), "lib", None);
        assert!(result.is_err());
    }
}
