use mollow_core::{CapabilityStatus, ChangeClassification};

use crate::ReportLanguage;

pub(crate) fn title<'a>(language: ReportLanguage, english: &'a str, chinese: &'a str) -> &'a str {
    match language {
        ReportLanguage::English => english,
        ReportLanguage::Chinese => chinese,
    }
}

pub(crate) fn status_name(status: &CapabilityStatus, language: ReportLanguage) -> &'static str {
    match (status, language) {
        (CapabilityStatus::Available, ReportLanguage::English) => "available",
        (CapabilityStatus::Unsupported, ReportLanguage::English) => "unsupported",
        (CapabilityStatus::PermissionDenied, ReportLanguage::English) => "permission_denied",
        (CapabilityStatus::Unavailable, ReportLanguage::English) => "unavailable",
        (CapabilityStatus::Error, ReportLanguage::English) => "error",
        (CapabilityStatus::Available, ReportLanguage::Chinese) => "可用",
        (CapabilityStatus::Unsupported, ReportLanguage::Chinese) => "不支持",
        (CapabilityStatus::PermissionDenied, ReportLanguage::Chinese) => "权限不足",
        (CapabilityStatus::Unavailable, ReportLanguage::Chinese) => "不可用",
        (CapabilityStatus::Error, ReportLanguage::Chinese) => "错误",
    }
}

pub(crate) fn classification_name(
    classification: ChangeClassification,
    language: ReportLanguage,
) -> &'static str {
    match (classification, language) {
        (ChangeClassification::Improvement, ReportLanguage::English) => "Improvement",
        (ChangeClassification::Regression, ReportLanguage::English) => "Regression",
        (ChangeClassification::Stable, ReportLanguage::English) => "Stable",
        (ChangeClassification::NotComparable, ReportLanguage::English) => "Not comparable",
        (ChangeClassification::Unavailable, ReportLanguage::English) => "Unavailable",
        (ChangeClassification::Improvement, ReportLanguage::Chinese) => "提升",
        (ChangeClassification::Regression, ReportLanguage::Chinese) => "回退",
        (ChangeClassification::Stable, ReportLanguage::Chinese) => "稳定",
        (ChangeClassification::NotComparable, ReportLanguage::Chinese) => "不可比较",
        (ChangeClassification::Unavailable, ReportLanguage::Chinese) => "不可用",
    }
}

pub(crate) fn yes_no(value: bool, language: ReportLanguage) -> &'static str {
    match (value, language) {
        (true, ReportLanguage::English) => "yes",
        (false, ReportLanguage::English) => "no",
        (true, ReportLanguage::Chinese) => "是",
        (false, ReportLanguage::Chinese) => "否",
    }
}
