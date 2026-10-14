import { Routes, Route } from 'react-router-dom';
import BasicLayout from '../layouts/BasicLayout';
import JobPage from '../pages/job';
import TaskPage from '../pages/task';

export default function AppRouter() {
  return (
    <Routes>
      <Route path="/" element={<BasicLayout />}>
        <Route index element={<JobPage />} />
        <Route path="job" element={<JobPage />} />
        <Route path="task" element={<TaskPage />} />
      </Route>
    </Routes>
  );
}
