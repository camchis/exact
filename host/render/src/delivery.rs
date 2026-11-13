use exact_runner::delivery::Delivery;
use std::path::Path;

pub(crate) fn beside(plan: &Path) -> std::io::Result<Delivery> {
    let bytes = match std::fs::read(plan.with_file_name("app.version.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Delivery::default()),
        Err(e) => return Err(e),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let field = |name| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                std::io::Error::other(format!("app.version.json: {name} must be a string"))
            })
    };
    // A file from before `production` was written names none: a development build's answer.
    let production = match value.get("production") {
        None => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(_) => {
            return Err(std::io::Error::other(
                "app.version.json: production must be a boolean",
            ))
        }
    };
    Ok(Delivery {
        version: field("version")?,
        build_number: field("buildNumber")?,
        production,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::Delivery;
    use crate::*;

    struct NoData;
    impl DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn delivery_precedes_child_initializers() {
        let delivery = Delivery {
            version: "2.3.4".into(),
            build_number: "56".into(),
            ..Default::default()
        };
        let plan = contract::compile(
            r#"
routes nav
  home "/" render=build activate=never
shape Build
  version: string
component App
  resource build = exactDelivery() as shape Build
  view
    Version(start=build.version)
component Version
  props
    start: string
  state shown = start
  state first = performanceNow()
  view
    text `${shown} at ${first}`
"#,
        )
        .unwrap();
        let page = settle_at(
            &plan,
            &|| NoData,
            Default::default(),
            "/",
            DEADLINE,
            false,
            1700000123456.,
            &delivery,
            None,
        )
        .unwrap();
        let document = exact_web::document::project(&page.runner).unwrap();
        assert!(
            document.root.contains(">2.3.4 at 1700000123456<"),
            "{}",
            document.root
        );
        assert_eq!(page.runner.now_ms(), 1700000123456.);
        let rendered = render_with_at(
            &plan,
            &|| NoData,
            Default::default(),
            "/",
            &Site::default(),
            DEADLINE,
            Ids::Any,
            Projection::Auto,
            1700000123456.,
            &delivery,
        )
        .unwrap();
        assert!(
            rendered.document.root.contains(">2.3.4 at 1700000123456<"),
            "{}",
            rendered.document.root
        );
        assert_eq!(rendered.activate, exact_plan::ActivatePolicy::Never);
        retire(page, &|| NoData);
    }

    #[test]
    fn production_is_read_beside_the_plan_and_absent_is_a_development_build() {
        let dir =
            std::env::temp_dir().join(format!("exact-render-production-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plan = dir.join("app.plan");
        let read = |json: &str| {
            std::fs::write(dir.join("app.version.json"), json).unwrap();
            super::beside(&plan)
        };
        let built = read(r#"{"version":"2.3.4","buildNumber":"56","production":true}"#).unwrap();
        assert_eq!((built.version.as_str(), built.production), ("2.3.4", true));
        assert!(
            !read(r#"{"version":"","buildNumber":""}"#)
                .unwrap()
                .production
        );
        assert_eq!(
            read(r#"{"version":"","buildNumber":"","production":"yes"}"#)
                .unwrap_err()
                .to_string(),
            "app.version.json: production must be a boolean"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
