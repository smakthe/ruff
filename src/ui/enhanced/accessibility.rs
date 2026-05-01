use crate::error::EnhancedError;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use std::collections::HashMap;

/// Accessibility manager for screen reader compatibility and keyboard navigation
pub struct AccessibilityManager {
    /// Current focus element ID
    current_focus: Option<String>,
    /// Map of element IDs to their accessibility labels
    aria_labels: HashMap<String, String>,
    /// Map of element IDs to their roles
    aria_roles: HashMap<String, AriaRole>,
    /// Map of element IDs to their descriptions
    aria_descriptions: HashMap<String, String>,
    /// Navigation order for keyboard-only navigation
    tab_order: Vec<String>,
    /// Current tab index
    current_tab_index: usize,
    /// Whether high contrast mode is enabled
    high_contrast_mode: bool,
    /// Screen reader announcements queue
    announcements: Vec<String>,
}

/// ARIA roles for UI elements
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AriaRole {
    Button,
    TextBox,
    List,
    ListItem,
    Menu,
    MenuItem,
    Dialog,
    Alert,
    Status,
    Region,
    Main,
    Navigation,
    Banner,
    Complementary,
    ContentInfo,
    Application,
}

/// Accessibility validation result
#[derive(Debug, Clone)]
pub struct AccessibilityValidation {
    pub is_valid: bool,
    pub issues: Vec<AccessibilityIssue>,
    pub wcag_level: WcagLevel,
}

/// WCAG compliance levels
#[derive(Debug, Clone, PartialEq)]
pub enum WcagLevel {
    A,
    AA,
    AAA,
}

/// Accessibility issue types
#[derive(Debug, Clone)]
pub struct AccessibilityIssue {
    pub element_id: String,
    pub issue_type: AccessibilityIssueType,
    pub description: String,
    pub severity: IssueSeverity,
    pub wcag_guideline: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityIssueType {
    MissingLabel,
    InsufficientContrast,
    MissingRole,
    InvalidTabOrder,
    MissingDescription,
    KeyboardTrap,
    FocusNotVisible,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Navigation result for keyboard navigation
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationResult {
    Moved(String),
    AtBoundary,
    NoFocusableElements,
}

impl AccessibilityManager {
    pub fn new() -> Self {
        Self {
            current_focus: None,
            aria_labels: HashMap::new(),
            aria_roles: HashMap::new(),
            aria_descriptions: HashMap::new(),
            tab_order: Vec::new(),
            current_tab_index: 0,
            high_contrast_mode: false,
            announcements: Vec::new(),
        }
    }

    /// Register an element with accessibility information
    pub fn register_element(
        &mut self,
        id: String,
        label: String,
        role: AriaRole,
        description: Option<String>,
    ) -> Result<(), EnhancedError> {
        self.aria_labels.insert(id.clone(), label);
        self.aria_roles.insert(id.clone(), role);

        if let Some(desc) = description {
            self.aria_descriptions.insert(id.clone(), desc);
        }

        // Add to tab order if it's a focusable element
        if self.is_focusable_role(&role) {
            self.tab_order.push(id);
        }

        Ok(())
    }

    /// Remove an element from accessibility tracking
    pub fn unregister_element(&mut self, id: &str) {
        self.aria_labels.remove(id);
        self.aria_roles.remove(id);
        self.aria_descriptions.remove(id);
        self.tab_order.retain(|x| x != id);

        // Update current focus if the removed element was focused
        if self.current_focus.as_ref() == Some(&id.to_string()) {
            self.current_focus = None;
        }
    }

    /// Set focus to a specific element
    pub fn set_focus(&mut self, element_id: String) -> Result<(), EnhancedError> {
        if !self.aria_labels.contains_key(&element_id) {
            return Err(EnhancedError::unknown(format!(
                "Element {} not registered",
                element_id
            )));
        }

        self.current_focus = Some(element_id.clone());

        // Update tab index
        if let Some(index) = self.tab_order.iter().position(|x| x == &element_id) {
            self.current_tab_index = index;
        }

        // Announce focus change to screen reader
        if let Some(label) = self.aria_labels.get(&element_id) {
            self.announce(format!("Focused: {}", label));
        }

        Ok(())
    }

    /// Navigate to the next focusable element
    pub fn navigate_next(&mut self) -> NavigationResult {
        if self.tab_order.is_empty() {
            return NavigationResult::NoFocusableElements;
        }

        if self.current_tab_index >= self.tab_order.len() - 1 {
            return NavigationResult::AtBoundary;
        }

        self.current_tab_index += 1;
        let next_element = self.tab_order[self.current_tab_index].clone();
        self.current_focus = Some(next_element.clone());

        if let Some(label) = self.aria_labels.get(&next_element) {
            self.announce(format!("Focused: {}", label));
        }

        NavigationResult::Moved(next_element)
    }

    /// Navigate to the previous focusable element
    pub fn navigate_previous(&mut self) -> NavigationResult {
        if self.tab_order.is_empty() {
            return NavigationResult::NoFocusableElements;
        }

        if self.current_tab_index == 0 {
            return NavigationResult::AtBoundary;
        }

        self.current_tab_index -= 1;
        let prev_element = self.tab_order[self.current_tab_index].clone();
        self.current_focus = Some(prev_element.clone());

        if let Some(label) = self.aria_labels.get(&prev_element) {
            self.announce(format!("Focused: {}", label));
        }

        NavigationResult::Moved(prev_element)
    }

    /// Get the currently focused element
    pub fn get_current_focus(&self) -> Option<&String> {
        self.current_focus.as_ref()
    }

    /// Enable or disable high contrast mode
    pub fn set_high_contrast_mode(&mut self, enabled: bool) {
        self.high_contrast_mode = enabled;
        let message = if enabled {
            "High contrast mode enabled"
        } else {
            "High contrast mode disabled"
        };
        self.announce(message.to_string());
    }

    /// Check if high contrast mode is enabled
    pub fn is_high_contrast_mode(&self) -> bool {
        self.high_contrast_mode
    }

    /// Add an announcement for screen readers
    pub fn announce(&mut self, message: String) {
        self.announcements.push(message);
    }

    /// Get and clear pending announcements
    pub fn get_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.announcements)
    }

    /// Get accessibility information for an element
    pub fn get_element_info(&self, element_id: &str) -> Option<ElementAccessibilityInfo> {
        let label = self.aria_labels.get(element_id)?;
        let role = self.aria_roles.get(element_id)?;
        let description = self.aria_descriptions.get(element_id);

        Some(ElementAccessibilityInfo {
            id: element_id.to_string(),
            label: label.clone(),
            role: role.clone(),
            description: description.cloned(),
            is_focused: self.current_focus.as_ref() == Some(&element_id.to_string()),
        })
    }

    /// Validate accessibility compliance
    pub fn validate_accessibility(&self) -> AccessibilityValidation {
        let mut issues = Vec::new();

        // Check for missing labels
        for (id, role) in &self.aria_roles {
            if !self.aria_labels.contains_key(id) {
                issues.push(AccessibilityIssue {
                    element_id: id.clone(),
                    issue_type: AccessibilityIssueType::MissingLabel,
                    description: "Element is missing an accessible label".to_string(),
                    severity: IssueSeverity::Critical,
                    wcag_guideline: "WCAG 2.1 - 4.1.2 Name, Role, Value".to_string(),
                });
            }

            // Check for missing descriptions on complex elements
            if self.requires_description(role) && !self.aria_descriptions.contains_key(id) {
                issues.push(AccessibilityIssue {
                    element_id: id.clone(),
                    issue_type: AccessibilityIssueType::MissingDescription,
                    description: "Complex element is missing a description".to_string(),
                    severity: IssueSeverity::Medium,
                    wcag_guideline: "WCAG 2.1 - 3.3.2 Labels or Instructions".to_string(),
                });
            }
        }

        // Check tab order
        if self.tab_order.is_empty() && !self.aria_roles.is_empty() {
            issues.push(AccessibilityIssue {
                element_id: "global".to_string(),
                issue_type: AccessibilityIssueType::InvalidTabOrder,
                description: "No focusable elements in tab order".to_string(),
                severity: IssueSeverity::High,
                wcag_guideline: "WCAG 2.1 - 2.4.3 Focus Order".to_string(),
            });
        }

        // Determine WCAG level based on issues
        let wcag_level = if issues.iter().any(|i| i.severity == IssueSeverity::Critical) {
            WcagLevel::A
        } else if issues.iter().any(|i| i.severity == IssueSeverity::High) {
            WcagLevel::AA
        } else {
            WcagLevel::AAA
        };

        AccessibilityValidation {
            is_valid: issues.is_empty(),
            issues,
            wcag_level,
        }
    }

    /// Create high contrast styles for an element
    pub fn get_high_contrast_style(&self, base_style: Style, element_type: &str) -> Style {
        if !self.high_contrast_mode {
            return base_style;
        }

        match element_type {
            "focused" => Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(ratatui::style::Modifier::BOLD)
                .add_modifier(ratatui::style::Modifier::UNDERLINED),
            "button" => Style::default()
                .bg(Color::Black)
                .fg(Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD),
            "text" => Style::default().bg(Color::White).fg(Color::Black),
            "error" => Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD),
            "success" => Style::default()
                .bg(Color::Green)
                .fg(Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD),
            _ => Style::default().bg(Color::White).fg(Color::Black),
        }
    }

    /// Create accessible block with proper borders and focus indication
    pub fn create_accessible_block<'a>(&self, title: &'a str, element_id: &str) -> Block<'a> {
        let mut block = Block::default().title(title).borders(Borders::ALL);

        if self.current_focus.as_ref() == Some(&element_id.to_string()) {
            block = block.border_style(self.get_high_contrast_style(Style::default(), "focused"));
        }

        block
    }

    /// Check if a role is focusable
    fn is_focusable_role(&self, role: &AriaRole) -> bool {
        matches!(
            role,
            AriaRole::Button
                | AriaRole::TextBox
                | AriaRole::MenuItem
                | AriaRole::List
                | AriaRole::Dialog
        )
    }

    /// Check if a role requires a description
    fn requires_description(&self, role: &AriaRole) -> bool {
        matches!(
            role,
            AriaRole::Dialog | AriaRole::Alert | AriaRole::Application | AriaRole::Region
        )
    }
}

/// Accessibility information for a UI element
#[derive(Debug, Clone)]
pub struct ElementAccessibilityInfo {
    pub id: String,
    pub label: String,
    pub role: AriaRole,
    pub description: Option<String>,
    pub is_focused: bool,
}

impl Default for AccessibilityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AriaRole {
    /// Convert role to string for screen readers
    pub fn to_string(&self) -> &'static str {
        match self {
            AriaRole::Button => "button",
            AriaRole::TextBox => "textbox",
            AriaRole::List => "list",
            AriaRole::ListItem => "listitem",
            AriaRole::Menu => "menu",
            AriaRole::MenuItem => "menuitem",
            AriaRole::Dialog => "dialog",
            AriaRole::Alert => "alert",
            AriaRole::Status => "status",
            AriaRole::Region => "region",
            AriaRole::Main => "main",
            AriaRole::Navigation => "navigation",
            AriaRole::Banner => "banner",
            AriaRole::Complementary => "complementary",
            AriaRole::ContentInfo => "contentinfo",
            AriaRole::Application => "application",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_manager_creation() {
        let manager = AccessibilityManager::new();
        assert!(manager.get_current_focus().is_none());
        assert!(!manager.is_high_contrast_mode());
    }

    #[test]
    fn test_element_registration() {
        let mut manager = AccessibilityManager::new();

        let result = manager.register_element(
            "button1".to_string(),
            "Submit Button".to_string(),
            AriaRole::Button,
            Some("Click to submit the form".to_string()),
        );

        assert!(result.is_ok());

        let info = manager.get_element_info("button1").unwrap();
        assert_eq!(info.label, "Submit Button");
        assert_eq!(info.role, AriaRole::Button);
        assert_eq!(
            info.description,
            Some("Click to submit the form".to_string())
        );
    }

    #[test]
    fn test_focus_management() {
        let mut manager = AccessibilityManager::new();

        manager
            .register_element(
                "button1".to_string(),
                "Button 1".to_string(),
                AriaRole::Button,
                None,
            )
            .unwrap();

        manager
            .register_element(
                "button2".to_string(),
                "Button 2".to_string(),
                AriaRole::Button,
                None,
            )
            .unwrap();

        // Set focus
        assert!(manager.set_focus("button1".to_string()).is_ok());
        assert_eq!(manager.get_current_focus(), Some(&"button1".to_string()));

        // Navigate next
        let result = manager.navigate_next();
        assert_eq!(result, NavigationResult::Moved("button2".to_string()));
        assert_eq!(manager.get_current_focus(), Some(&"button2".to_string()));

        // Navigate at boundary
        let result = manager.navigate_next();
        assert_eq!(result, NavigationResult::AtBoundary);

        // Navigate previous
        let result = manager.navigate_previous();
        assert_eq!(result, NavigationResult::Moved("button1".to_string()));
    }

    #[test]
    fn test_high_contrast_mode() {
        let mut manager = AccessibilityManager::new();

        assert!(!manager.is_high_contrast_mode());

        manager.set_high_contrast_mode(true);
        assert!(manager.is_high_contrast_mode());

        let style = manager.get_high_contrast_style(Style::default(), "focused");
        // Style should be modified for high contrast
        assert_ne!(style, Style::default());
    }

    #[test]
    fn test_announcements() {
        let mut manager = AccessibilityManager::new();

        manager.announce("Test announcement".to_string());
        manager.announce("Another announcement".to_string());

        let announcements = manager.get_announcements();
        assert_eq!(announcements.len(), 2);
        assert_eq!(announcements[0], "Test announcement");
        assert_eq!(announcements[1], "Another announcement");

        // Announcements should be cleared after getting them
        let empty_announcements = manager.get_announcements();
        assert!(empty_announcements.is_empty());
    }

    #[test]
    fn test_accessibility_validation() {
        let mut manager = AccessibilityManager::new();

        // Register element without label (should create issue)
        manager
            .aria_roles
            .insert("bad_element".to_string(), AriaRole::Button);

        // Register proper element
        manager
            .register_element(
                "good_element".to_string(),
                "Good Button".to_string(),
                AriaRole::Button,
                None,
            )
            .unwrap();

        let validation = manager.validate_accessibility();
        assert!(!validation.is_valid);
        assert!(!validation.issues.is_empty());

        // Should have critical issue for missing label
        assert!(validation
            .issues
            .iter()
            .any(|i| i.issue_type == AccessibilityIssueType::MissingLabel
                && i.severity == IssueSeverity::Critical));
    }

    #[test]
    fn test_element_unregistration() {
        let mut manager = AccessibilityManager::new();

        manager
            .register_element(
                "button1".to_string(),
                "Button 1".to_string(),
                AriaRole::Button,
                None,
            )
            .unwrap();

        manager.set_focus("button1".to_string()).unwrap();
        assert!(manager.get_element_info("button1").is_some());
        assert_eq!(manager.get_current_focus(), Some(&"button1".to_string()));

        manager.unregister_element("button1");
        assert!(manager.get_element_info("button1").is_none());
        assert!(manager.get_current_focus().is_none());
    }

    #[test]
    fn test_aria_role_to_string() {
        assert_eq!(AriaRole::Button.to_string(), "button");
        assert_eq!(AriaRole::TextBox.to_string(), "textbox");
        assert_eq!(AriaRole::Dialog.to_string(), "dialog");
        assert_eq!(AriaRole::Alert.to_string(), "alert");
    }

    #[test]
    fn test_navigation_with_no_elements() {
        let mut manager = AccessibilityManager::new();

        let result = manager.navigate_next();
        assert_eq!(result, NavigationResult::NoFocusableElements);

        let result = manager.navigate_previous();
        assert_eq!(result, NavigationResult::NoFocusableElements);
    }
}
