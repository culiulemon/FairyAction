use crate::context::ActionContext;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Oneshot,
    Persistent,
    Both,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub param_type: ParamType,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    String,
    Int,
    Bool,
    Array,
}

impl Param {
    pub fn string(name: &str) -> Self {
        Self {
            name: name.to_string(),
            param_type: ParamType::String,
            required: false,
            description: None,
            default: None,
        }
    }
    pub fn int(name: &str) -> Self {
        Self {
            name: name.to_string(),
            param_type: ParamType::Int,
            required: false,
            description: None,
            default: None,
        }
    }
    pub fn bool_val(name: &str) -> Self {
        Self {
            name: name.to_string(),
            param_type: ParamType::Bool,
            required: false,
            description: None,
            default: None,
        }
    }
    pub fn array(name: &str) -> Self {
        Self {
            name: name.to_string(),
            param_type: ParamType::Array,
            required: false,
            description: None,
            default: None,
        }
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn desc(mut self, d: &str) -> Self {
        self.description = Some(d.to_string());
        self
    }
    pub fn default_val(mut self, v: Value) -> Self {
        self.default = Some(v);
        self
    }
}

pub type HandlerFn = Box<dyn Fn(Value, &ActionContext) -> anyhow::Result<Value> + Send + Sync>;

pub struct Action {
    pub name: String,
    pub description: Option<String>,
    pub params: Vec<Param>,
    pub handler: HandlerFn,
}

impl Action {
    pub fn new(
        name: &str,
        handler: impl Fn(Value, &ActionContext) -> anyhow::Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            params: vec![],
            handler: Box::new(handler),
        }
    }
    pub fn description(mut self, d: &str) -> Self {
        self.description = Some(d.to_string());
        self
    }
    pub fn param(mut self, p: Param) -> Self {
        self.params.push(p);
        self
    }
}

pub struct Domain {
    pub name: String,
    pub description: Option<String>,
    pub actions: Vec<Action>,
}

impl Domain {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            actions: vec![],
        }
    }
    pub fn description(mut self, d: &str) -> Self {
        self.description = Some(d.to_string());
        self
    }
    pub fn action(mut self, a: Action) -> Self {
        self.actions.push(a);
        self
    }
}

pub struct App {
    pub name: String,
    pub version: String,
    pub lifecycle: Lifecycle,
    pub domains: Vec<Domain>,
}

impl App {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            version: "1.0.0".to_string(),
            lifecycle: Lifecycle::Both,
            domains: vec![],
        }
    }

    pub fn name(mut self, n: &str) -> Self {
        self.name = n.to_string();
        self
    }
    pub fn version(mut self, v: &str) -> Self {
        self.version = v.to_string();
        self
    }
    pub fn lifecycle(mut self, l: Lifecycle) -> Self {
        self.lifecycle = l;
        self
    }
    pub fn domain(mut self, d: Domain) -> Self {
        self.domains.push(d);
        self
    }

    pub fn run(self) {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 && args[1] == "--capabilities" {
            self.print_capabilities();
            return;
        }
        if args.len() > 1 && args[1] == "--serve" {
            self.run_persistent();
            return;
        }
        self.run_oneshot(&args[1..]);
    }

    pub(crate) fn print_capabilities(&self) {
        let caps = self.build_capabilities_json();
        println!("{}", serde_json::to_string_pretty(&caps).unwrap());
    }

    pub(crate) fn build_capabilities_json(&self) -> Value {
        let domains: Vec<Value> = self
            .domains
            .iter()
            .map(|d| {
                let actions: Vec<Value> = d
                    .actions
                    .iter()
                    .map(|a| {
                        let params: serde_json::Map<String, Value> = a
                            .params
                            .iter()
                            .map(|p| {
                                let mut m = serde_json::Map::new();
                                m.insert(
                                    "类型".to_string(),
                                    match p.param_type {
                                        ParamType::String => Value::String("字符串".to_string()),
                                        ParamType::Int => Value::String("整数".to_string()),
                                        ParamType::Bool => Value::String("布尔".to_string()),
                                        ParamType::Array => Value::String("数组".to_string()),
                                    },
                                );
                                if p.required {
                                    m.insert("必填".to_string(), Value::Bool(true));
                                }
                                if let Some(desc) = &p.description {
                                    m.insert("描述".to_string(), Value::String(desc.clone()));
                                }
                                if let Some(default) = &p.default {
                                    m.insert("默认".to_string(), default.clone());
                                }
                                (p.name.clone(), Value::Object(m))
                            })
                            .collect();
                        let mut obj = serde_json::Map::new();
                        obj.insert("名称".to_string(), Value::String(a.name.clone()));
                        if let Some(desc) = &a.description {
                            obj.insert("描述".to_string(), Value::String(desc.clone()));
                        }
                        obj.insert("参数".to_string(), Value::Object(params));
                        Value::Object(obj)
                    })
                    .collect();
                let mut obj = serde_json::Map::new();
                obj.insert("名称".to_string(), Value::String(d.name.clone()));
                if let Some(desc) = &d.description {
                    obj.insert("描述".to_string(), Value::String(desc.clone()));
                }
                obj.insert("动作".to_string(), Value::Array(actions));
                Value::Object(obj)
            })
            .collect();
        let mut result = serde_json::Map::new();
        result.insert("能力域".to_string(), Value::Array(domains));
        Value::Object(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_builder() {
        let p = Param::string("name")
            .required()
            .desc("your name")
            .default_val(Value::String("world".to_string()));
        assert_eq!(p.name, "name");
        assert_eq!(p.param_type, ParamType::String);
        assert!(p.required);
        assert_eq!(p.description.as_deref(), Some("your name"));
        assert_eq!(p.default, Some(Value::String("world".to_string())));
    }

    #[test]
    fn test_param_types() {
        let p1 = Param::int("count");
        assert_eq!(p1.param_type, ParamType::Int);
        assert!(!p1.required);

        let p2 = Param::bool_val("flag");
        assert_eq!(p2.param_type, ParamType::Bool);

        let p3 = Param::array("items");
        assert_eq!(p3.param_type, ParamType::Array);
    }

    #[test]
    fn test_app_builder() {
        let app = App::new()
            .name("test-app")
            .version("2.0.0")
            .lifecycle(Lifecycle::Oneshot);
        assert_eq!(app.name, "test-app");
        assert_eq!(app.version, "2.0.0");
        assert_eq!(app.lifecycle, Lifecycle::Oneshot);
    }

    #[test]
    fn test_build_capabilities_json() {
        let app = App::new()
            .name("demo")
            .domain(
                Domain::new("文件操作")
                    .description("文件相关操作")
                    .action(
                        Action::new(
                            "读取文件",
                            |_, _| Ok(Value::String("ok".to_string())),
                        )
                        .description("读取文件内容")
                        .param(Param::string("路径").required().desc("文件路径"))
                        .param(Param::bool_val("verbose").default_val(Value::Bool(false))),
                    ),
            );

        let caps = app.build_capabilities_json();

        let domains = caps.get("能力域").unwrap().as_array().unwrap();
        assert_eq!(domains.len(), 1);

        let domain = &domains[0];
        assert_eq!(domain.get("名称").unwrap().as_str().unwrap(), "文件操作");
        assert_eq!(
            domain.get("描述").unwrap().as_str().unwrap(),
            "文件相关操作"
        );

        let actions = domain.get("动作").unwrap().as_array().unwrap();
        assert_eq!(actions.len(), 1);

        let action = &actions[0];
        assert_eq!(action.get("名称").unwrap().as_str().unwrap(), "读取文件");
        assert_eq!(action.get("描述").unwrap().as_str().unwrap(), "读取文件内容");

        let params = action.get("参数").unwrap().as_object().unwrap();
        let path_param = params.get("路径").unwrap().as_object().unwrap();
        assert_eq!(path_param.get("类型").unwrap().as_str().unwrap(), "字符串");
        assert_eq!(path_param.get("必填").unwrap().as_bool().unwrap(), true);
        assert_eq!(
            path_param.get("描述").unwrap().as_str().unwrap(),
            "文件路径"
        );

        let verbose_param = params.get("verbose").unwrap().as_object().unwrap();
        assert_eq!(verbose_param.get("类型").unwrap().as_str().unwrap(), "布尔");
        assert_eq!(verbose_param.get("默认").unwrap().as_bool().unwrap(), false);
        assert!(verbose_param.get("必填").is_none());
    }

    #[test]
    fn test_multiple_domains() {
        let app = App::new()
            .name("multi")
            .domain(Domain::new("域A").action(Action::new("动作1", |_, _| Ok(Value::Null))))
            .domain(Domain::new("域B").action(Action::new("动作2", |_, _| Ok(Value::Null))));

        let caps = app.build_capabilities_json();
        let domains = caps.get("能力域").unwrap().as_array().unwrap();
        assert_eq!(domains.len(), 2);
    }
}
