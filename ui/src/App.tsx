import { AnimatePresence, motion } from 'framer-motion';
import { useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { SettingsDialog, ToolchainsDialog } from './components/Dialogs';
import { StoreProvider, useStore } from './state';
import { AnalyzeView } from './views/AnalyzeView';
import { BuildView } from './views/BuildView';
import { ConfigureView } from './views/ConfigureView';
import { ConvertView } from './views/ConvertView';
import { ReportView } from './views/ReportView';
import { SourceView } from './views/SourceView';
import { TargetsView } from './views/TargetsView';

const VIEWS = {
  source: SourceView, analyze: AnalyzeView, targets: TargetsView, configure: ConfigureView,
  convert: ConvertView, build: BuildView, report: ReportView,
};

function Shell() {
  const { step } = useStore();
  const [tools, setTools] = useState(false);
  const View = VIEWS[step];
  return (
    <>
      <div className="aurora"><i /><i /><i /></div>
      <div className="gridlines" />
      <div className="grain" />
      <div className="titlebar-drag" data-tauri-drag-region />
      <div className="shell">
        <Sidebar onToolchains={() => setTools(true)} />
        <main className="main">
          <AnimatePresence mode="wait">
            <motion.div key={step} className="page" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: .2 }}>
              <View />
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
      <SettingsDialog />
      <ToolchainsDialog open={tools} onClose={() => setTools(false)} />
    </>
  );
}

export default function App() {
  return <StoreProvider><Shell /></StoreProvider>;
}
