import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import NodeTaskTrackingCard from './NodeTaskTrackingCard'

describe('NodeTaskTrackingCard', () => {
  it('renders loading state', () => {
    render(<NodeTaskTrackingCard isLoading={true} error={null} task={null} />)
    expect(screen.getByTestId('task-loading')).toHaveTextContent('Loading task...')
  })

  it('renders error state', () => {
    render(<NodeTaskTrackingCard isLoading={false} error="Network failed" task={null} />)
    expect(screen.getByTestId('task-error')).toHaveTextContent('Error loading task: Network failed')
  })

  it('renders empty state', () => {
    render(<NodeTaskTrackingCard isLoading={false} error={null} task={null} />)
    expect(screen.getByTestId('task-empty')).toHaveTextContent('No latest task found for this node.')
  })

  it('renders task details with safe request_summary fields', () => {
    const task = {
      install_task_id: 'task-123',
      node_id: 'node-1',
      task_state: 'COMPLETED',
      started_at: '2026-06-07T00:00:00Z',
      retryable: false,
      request_summary: {
        host: '10.0.0.1',
        ssh_port: 22,
        username: 'admin',
        rsagent_package_url: 'http://pkg/agent.tar.gz',
        install_root: '/opt/rsagent',
        labels: ['web'],
        plugin_names: ['monitor']
      }
    }

    render(<NodeTaskTrackingCard isLoading={false} error={null} task={task} />)

    expect(screen.getByTestId('task-tracking-id')).toHaveTextContent('task-123')
    expect(screen.getByTestId('task-tracking-state')).toHaveTextContent('COMPLETED')
    expect(screen.getByTestId('task-tracking-started')).toHaveTextContent('2026-06-07T00:00:00Z')
    
    // Check safe fields
    expect(screen.getByTestId('task-summary-host')).toHaveTextContent('10.0.0.1')
    expect(screen.getByTestId('task-summary-port')).toHaveTextContent('22')
    expect(screen.getByTestId('task-summary-user')).toHaveTextContent('admin')
    expect(screen.getByTestId('task-summary-pkg')).toHaveTextContent('http://pkg/agent.tar.gz')
    expect(screen.getByTestId('task-summary-root')).toHaveTextContent('/opt/rsagent')
    expect(screen.getByTestId('task-summary-labels')).toHaveTextContent('web')
    expect(screen.getByTestId('task-summary-plugins')).toHaveTextContent('monitor')
  })

  it('renders error_code and error_message when present', () => {
    const task = {
      install_task_id: 'task-err',
      node_id: 'node-1',
      task_state: 'FAILED',
      started_at: '2026-06-07T00:00:00Z',
      retryable: true,
      error_code: 'SSH_AUTH_FAILED',
      error_message: 'Invalid credentials'
    }

    render(<NodeTaskTrackingCard isLoading={false} error={null} task={task} />)

    expect(screen.getByTestId('task-tracking-errcode')).toHaveTextContent('SSH_AUTH_FAILED')
    expect(screen.getByTestId('task-tracking-errmsg')).toHaveTextContent('Invalid credentials')
  })
})
