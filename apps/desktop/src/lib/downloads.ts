const PREPARING = new Set(['preparing', 'awaiting_review', 'interrupted', 'failed']);
const TRANSFERRING = new Set(['preparing', 'checking', 'downloading', 'starting', 'sharing']);
const BROWSER = new Set(['waiting', 'downloading']);

/** Counts work the Games navigation should surface. Registered and completed jobs are quiet. */
export function activityCounts(
  jobs: { state: string }[],
  transfers: { state: string }[],
  browser: { state: string }[],
) {
  return {
    preparation: jobs.filter((job) => PREPARING.has(job.state)).length,
    torrents: transfers.filter((item) => TRANSFERRING.has(item.state)).length,
    browser: browser.filter((job) => BROWSER.has(job.state)).length,
  };
}
