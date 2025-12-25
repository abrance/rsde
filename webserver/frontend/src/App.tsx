import { BrowserRouter as Router, Routes, Route } from 'react-router-dom'
import Layout from './components/Layout'
import HomePage from './pages/HomePage'
import RsyncPage from './pages/RsyncPage'
import RcPage from './pages/RcPage'
import OcrPage from './pages/OcrPage'
import './App.css'

function App() {
    return (
        <Router>
            <Layout>
                <Routes>
                    <Route path="/" element={<HomePage />} />
                    <Route path="/rsync" element={<RsyncPage />} />
                    <Route path="/rc" element={<RcPage />} />
                    <Route path="/ocr" element={<OcrPage />} />
                </Routes>
            </Layout>
        </Router>
    )
}

export default App
