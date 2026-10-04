use super::{advance, click};
use curcat::testing::UiLanguage;
use curcat_test_support::{AppHarness, app, click_image, harness, state};
use egui::{accesskit::Role, pos2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Files {
    dir: PathBuf,
    image: PathBuf,
    project: PathBuf,
}

impl Files {
    fn new(width: u32, color: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "curcat-ui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let files = Self {
            image: dir.join("chart.svg"),
            project: dir.join("session.curcat"),
            dir,
        };
        files.write_image(width, color);
        files
    }

    fn write_image(&self, width: u32, color: &str) {
        fs::write(&self.image, format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="480"><rect width="100%" height="100%" fill="{color}"/></svg>"#)).unwrap();
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

fn wait_for_workers(harness: &mut AppHarness) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app(harness).has_pending_tasks() {
        assert!(Instant::now() < deadline, "Fixture worker did not complete");
        harness.step();
        std::thread::sleep(Duration::from_millis(1));
    }
    advance(harness);
}

#[test]
fn project_warning_cancel_close_and_continue_preserve_or_apply_points() {
    for (language, title, cancel, proceed) in [
        (
            UiLanguage::En,
            "Project warnings",
            "Cancel",
            "Continue anyway",
        ),
        (
            UiLanguage::Ru,
            "Предупреждения проекта",
            "Отмена",
            "Продолжить",
        ),
    ] {
        for action in [cancel, "Close window", proceed] {
            let files = Files::new(640, "white");
            let mut harness = harness(2, language);
            harness
                .state_mut()
                .as_mut()
                .unwrap()
                .save_project(&files.project, &files.image);
            wait_for_workers(&mut harness);
            assert!(files.project.is_file());
            files.write_image(640, "black");
            click_image(&mut harness, pos2(350.0, 200.0));
            assert_eq!(state(&harness).points, 3);
            harness
                .state_mut()
                .as_mut()
                .unwrap()
                .load_project(&files.project);
            advance(&mut harness);
            assert_eq!(state(&harness).points, 3);
            let window = harness.get_by_role_and_label(Role::Window, title);
            assert!(window.query_by_label("Hide").is_none());
            window.get_by_label(action).click();
            advance(&mut harness);
            wait_for_workers(&mut harness);
            assert!(
                harness
                    .query_by_role_and_label(Role::Window, title)
                    .is_none()
            );
            assert_eq!(
                state(&harness).points,
                if action == proceed { 2 } else { 3 }
            );
        }
    }
}

#[test]
fn image_limits_reject_close_autoscale_and_ignore_have_expected_results() {
    for (language, title, reject, autoscale, ignore) in [
        (
            UiLanguage::En,
            "Image limits",
            "Reject load",
            "Autoscale to fit",
            "Ignore limits",
        ),
        (
            UiLanguage::Ru,
            "Лимиты изображения",
            "Отклонить загрузку",
            "Автоскейл до лимитов",
            "Игнорировать лимиты",
        ),
    ] {
        for action in [reject, "Close window", autoscale, ignore] {
            let files = Files::new(640, "white");
            let mut harness = harness(2, language);
            harness
                .state_mut()
                .as_mut()
                .unwrap()
                .load_image_with_limit(&files.image, 64);
            wait_for_workers(&mut harness);
            let window = harness.get_by_role_and_label(Role::Window, title);
            for label in [autoscale, ignore] {
                assert!(!window.get_by_label(label).accesskit_node().is_disabled());
            }
            window.get_by_label(action).click();
            advance(&mut harness);
            wait_for_workers(&mut harness);
            assert!(
                harness
                    .query_by_role_and_label(Role::Window, title)
                    .is_none()
            );
            let [width, height] = state(&harness).image_size.unwrap();
            if action == autoscale {
                assert!(width <= 64 && height <= 64);
                assert_eq!(state(&harness).points, 0);
            } else if action == ignore {
                assert!(width > 64);
                assert_eq!(state(&harness).points, 0);
            } else {
                assert_eq!([width, height], [640, 480]);
                assert_eq!(state(&harness).points, 2);
            }
        }
    }
}

#[test]
fn hard_image_limits_disable_both_accept_actions() {
    let files = Files::new(200_000, "white");
    let mut harness = harness(2, UiLanguage::En);
    harness
        .state_mut()
        .as_mut()
        .unwrap()
        .load_image_with_limit(&files.image, 64);
    wait_for_workers(&mut harness);
    for label in ["Autoscale to fit", "Ignore limits"] {
        assert!(harness.get_by_label(label).accesskit_node().is_disabled());
        click(&mut harness, label);
        assert!(!app(&harness).has_pending_tasks());
        assert_eq!(state(&harness).image_size, Some([640, 480]));
        assert!(
            harness
                .query_by_role_and_label(Role::Window, "Image limits")
                .is_some()
        );
    }
    click(&mut harness, "Close window");
    assert!(
        harness
            .query_by_role_and_label(Role::Window, "Image limits")
            .is_none()
    );
    assert_eq!(state(&harness).points, 2);
}
