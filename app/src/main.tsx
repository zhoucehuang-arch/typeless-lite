import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { Capsule } from './components/Capsule';
import './styles/global.css';

const params = new URLSearchParams(window.location.search);
const windowKind = params.get('window');
if (windowKind === 'capsule') {
  document.body.classList.add('capsule-window');
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    {windowKind === 'capsule' ? <Capsule /> : <App />}
  </React.StrictMode>,
);
