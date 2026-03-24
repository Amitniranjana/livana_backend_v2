-- Jobs table for career/job postings by associates
CREATE TABLE IF NOT EXISTS jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    associate_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT,
    location        TEXT,
    salary_range    TEXT,
    company_name    TEXT,
    job_type        TEXT,
    notice_period   TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_jobs_associate_id ON jobs(associate_id);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);

-- Job applications table
CREATE TABLE IF NOT EXISTS job_applications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id          UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resume_url      TEXT NOT NULL,
    cover_letter    TEXT NOT NULL,
    applied_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_job_applications_job_id ON job_applications(job_id);
CREATE INDEX IF NOT EXISTS idx_job_applications_user_id ON job_applications(user_id);
