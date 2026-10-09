//! The authored declarative markup of the demonstration form.

/// Default declarative UI markup template for the medical data entry screen.
pub const CLINICAL_SCREEN_XML: &str = r#"<screen id="main-screen" style="display: flex; flex-direction: column; height: 100%; background-color: #f0f4f8; padding: 20px; gap: 15px;">
  <div id="header" style="display: flex; flex-direction: column; gap: 8px; background: linear-gradient(135deg, #1a365d, #2c5282); padding: 12px; border-radius: 12px; box-shadow: 0 4px 12px #1a365d40;">
    <div id="header-title-row" style="display: flex; flex-direction: row; gap: 8px; align-items: center;">
      <div id="app-mark" aria-hidden="true" style="width: 24px; height: 24px;"></div>
      <text id="app-title" style="color: #ffffff; font-size: 22px; font-weight: bold;">METIS FORM DEMONSTRATION</text>
    </div>
    <text id="status-badge" style="color: #9ae6b4; font-size: 13px;">SYSTEM READY</text>
  </div>

  <nav id="application-navigation" aria-label="Application navigation" style="display: flex; flex-direction: column; gap: 8px; background-color: #e2e8f0; padding: 8px; border-radius: 10px;">
    <div id="application-toolbar" role="toolbar" aria-label="Application commands" style="display: flex; flex-direction: row; gap: 8px; align-items: center;">
      <button id="command-menu-toggle" aria-haspopup="menu" aria-expanded="false" aria-controls="command-menu" style="width: 120px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Commands</text>
      </button>
      <button id="command-focus-patient" aria-keyshortcuts="Alt+Shift+P" style="width: 144px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Focus patient</text>
      </button>
    </div>
    <div id="command-menu" popover-anchor="command-menu-toggle" role="menu" aria-label="Application commands" aria-hidden="true" style="display: none; flex-direction: column; gap: 6px; background-color: #ffffff; padding: 8px; border-width: 1px; border-color: #e2e8f0; border-radius: 10px; box-shadow: 0 8px 20px #0f172a33;">
      <button id="command-theme-dark" role="menuitem" aria-keyshortcuts="Alt+Shift+D" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Dark theme</text>
      </button>
      <button id="command-theme-system" role="menuitem" aria-keyshortcuts="Alt+Shift+S" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">System theme</text>
      </button>
    </div>
    <text id="command-status" role="status" aria-live="polite" style="color: #4a5568; font-size: 13px;">Commands ready</text>
  </nav>

  <card id="patient-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; border-radius: 12px; gap: 10px; box-shadow: 0 2px 10px #0f172a1f;">
    <text id="patient-heading" style="color: #2d3748; font-size: 16px; font-weight: bold;">Patient Demographics and Drug Prescription</text>
    <div id="row-patient" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-patient" role="textbox" aria-label="Patient ID" value="PT-9042-ALPHA" tabindex="0" style="color: #4a5568; font-size: 14px;">Patient ID: PT-9042-ALPHA</text>
    </div>
    <div id="row-weight" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-weight" style="color: #4a5568; font-size: 14px;">Weight: 72.50 kg</text>
    </div>
    <div id="row-conc" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-conc" style="color: #4a5568; font-size: 14px;">Drug Concentration: 4.00 mg/mL</text>
    </div>
    <div id="row-dose" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-dose" style="color: #4a5568; font-size: 14px;">Target Dose: 0.500 mcg/kg/min</text>
    </div>
    <div id="actions" style="display: flex; flex-direction: row; gap: 10px; margin: 10px 0 0 0; justify-content: center;">
      <button id="btn-calc" style="width: 360px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 16px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Submit calculation</text>
      </button>
    </div>
  </card>

  <card id="results-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; border-radius: 12px; gap: 8px; box-shadow: 0 2px 10px #0f172a1f;">
    <text id="results-heading" style="color: #2d3748; font-size: 16px; font-weight: bold;">Backend Calculation Output</text>
    <text id="output-rate" style="color: #2b6cb0; font-size: 18px; font-weight: bold;">Rate: Awaiting Backend Calculation...</text>
    <text id="output-status" style="color: #718096; font-size: 13px;">Safety Status: Idle</text>
    <text id="output-signature" style="color: #718096; font-size: 12px;">Backend MAC: None</text>
  </card>
</screen>"#;
