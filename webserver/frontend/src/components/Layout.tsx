import { Link, useLocation } from 'react-router-dom'
import './Layout.css'

interface LayoutProps {
    children: React.ReactNode
}

export default function Layout({ children }: LayoutProps) {
    const location = useLocation()

    const isActive = (path: string) => location.pathname === path

    return (
        <div className="layout">
            <header className="header">
                <div className="container">
                    <div className="header-content">
                        <Link to="/" className="logo">
                            <span className="logo-icon">🦀</span>
                            <span className="logo-text">RSDE</span>
                        </Link>
                        <nav className="nav">
                            <Link
                                to="/"
                                className={`nav-link ${isActive('/') ? 'active' : ''}`}
                            >
                                首页
                            </Link>
                            <Link
                                to="/rsync"
                                className={`nav-link ${isActive('/rsync') ? 'active' : ''}`}
                            >
                                Rsync
                            </Link>
                            <Link
                                to="/rc"
                                className={`nav-link ${isActive('/rc') ? 'active' : ''}`}
                            >
                                RC
                            </Link>
                            <Link
                                to="/ocr"
                                className={`nav-link ${isActive('/ocr') ? 'active' : ''}`}
                            >
                                OCR
                            </Link>
                        </nav>
                    </div>
                </div>
            </header>
            <main className="main">
                <div className="container">
                    {children}
                </div>
            </main>
            <footer className="footer">
                <div className="container">
                    <p>© 2025 RSDE - Rust Development Environment</p>
                </div>
            </footer>
        </div>
    )
}
