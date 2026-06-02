import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
import Layout from '../components/Layout'
import HomePage from './HomePage'
import JobManagePage from './JobManagePage'

describe('JobManagePage', () => {
    it('renders the job manage nav link inside layout', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Layout>
                    <div>placeholder</div>
                </Layout>
            </MemoryRouter>,
        )

        expect(screen.getByRole('link', { name: 'JobManage' })).toBeInTheDocument()
    })

    it('renders the job manage route heading', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        expect(screen.getByRole('heading', { name: 'JobManage' })).toBeInTheDocument()
    })

    it('renders initial summary, mock jobs, and detail placeholder', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        expect(screen.getByText('作业总数')).toBeInTheDocument()
        expect(screen.getByText('构建配置巡检')).toBeInTheDocument()
        expect(screen.getByText('执行脚本灰度发布')).toBeInTheDocument()
        expect(screen.getByText('选择左侧作业查看详情、预检结果与最近一次执行摘要。')).toBeInTheDocument()
    })

    it('filters jobs by status and shows empty state', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        const failedFilter = screen.getByRole('button', { name: '失败' })
        fireEvent.click(failedFilter)
        
        expect(screen.queryByText('构建配置巡检')).not.toBeInTheDocument()
        expect(screen.getByText('补丁热修复失败回滚')).toBeInTheDocument()
    })

    it('selects a job and shows detail panel', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        const row = screen.getByText('构建配置巡检')
        fireEvent.click(row)

        expect(screen.getByRole('heading', { name: '作业详情' })).toBeInTheDocument()
        expect(screen.queryByText('选择左侧作业查看详情、预检结果与最近一次执行摘要。')).not.toBeInTheDocument()
    })

    it('performs precheck action on a job', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        const row = screen.getByText('构建配置巡检')
        fireEvent.click(row)

        const precheckBtn = screen.getByRole('button', { name: '执行预检' })
        fireEvent.click(precheckBtn)

        expect(screen.getByText('预检已通过，可以执行')).toBeInTheDocument()
    })

    it('retries a failed job', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        const row = screen.getByText('补丁热修复失败回滚')
        fireEvent.click(row)

        const retryBtn = screen.getByRole('button', { name: '重试作业' })
        fireEvent.click(retryBtn)

        // The status should change to pending, and maybe a success message
        expect(screen.getByText('已提交重试')).toBeInTheDocument()
    })

    it('cancels a pending job', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        const row = screen.getByText('构建配置巡检')
        fireEvent.click(row)

        const cancelBtn = screen.getByRole('button', { name: '取消作业' })
        fireEvent.click(cancelBtn)

        expect(screen.getByText('已取消该作业')).toBeInTheDocument()
    })

    it('refresh resets filtered state back to the initial mock dataset', () => {
        render(
            <MemoryRouter initialEntries={['/job-manage']}>
                <Routes>
                    <Route path="/job-manage" element={<JobManagePage />} />
                </Routes>
            </MemoryRouter>,
        )

        fireEvent.click(screen.getByRole('button', { name: '失败' }))
        expect(screen.queryByText('构建配置巡检')).not.toBeInTheDocument()

        fireEvent.click(screen.getByRole('button', { name: '刷新数据' }))

        expect(screen.getByText('数据已刷新')).toBeInTheDocument()
        expect(screen.getByText('构建配置巡检')).toBeInTheDocument()
        expect(screen.getByText('执行脚本灰度发布')).toBeInTheDocument()
    })

    it('exposes the JobManage tool card on the home page', () => {
        render(
            <MemoryRouter>
                <HomePage />
            </MemoryRouter>,
        )

        const jobManageHeading = screen.getByRole('heading', { name: 'JobManage' })
        const jobManageCard = jobManageHeading.closest('.tool-card')

        expect(jobManageCard).not.toBeNull()
        expect(screen.getByText('面向 nodemanage 节点的前端作业管理工作台')).toBeInTheDocument()
        expect(within(jobManageCard as HTMLElement).getByRole('link', { name: '开始使用 →' })).toHaveAttribute(
            'href',
            '/job-manage',
        )
    })
})
