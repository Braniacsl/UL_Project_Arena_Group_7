-- 0. Local Dev Mock for Supabase Auth
CREATE SCHEMA IF NOT EXISTS auth;

CREATE TABLE IF NOT EXISTS auth.users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW() -- Added NOT NULL
);

-- 1. Profiles
CREATE TABLE IF NOT EXISTS public.profiles (
    id UUID PRIMARY KEY REFERENCES auth.users(id),
    email TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'student',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW() -- Added NOT NULL
);

-- 2. Projects
CREATE TABLE IF NOT EXISTS public.projects (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES auth.users(id) NOT NULL,
    author TEXT NOT NULL,                 
    title TEXT NOT NULL,
    abstract TEXT NOT NULL,
    cover_image TEXT NOT NULL,            
    video TEXT,                           
    report TEXT,                          
    report_is_public BOOLEAN NOT NULL DEFAULT FALSE,
    year INTEGER NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT FALSE,        -- Added NOT NULL
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),   -- Added NOT NULL
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()    -- Added NOT NULL
);

-- 3. Project Likes
CREATE TABLE IF NOT EXISTS public.project_likes (
    user_id UUID REFERENCES auth.users(id) NOT NULL,
    project_id UUID REFERENCES public.projects(id) NOT NULL,
    liked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),     -- Added NOT NULL
    PRIMARY KEY (user_id, project_id)
);

-- 4. Project Comments
CREATE TABLE IF NOT EXISTS public.project_comments (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id UUID REFERENCES auth.users(id) NOT NULL,
    project_id UUID REFERENCES public.projects(id) NOT NULL,
    comment TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),   -- Added NOT NULL
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()    -- Added NOT NULL
);

CREATE INDEX idx_projects_year ON public.projects(year);
CREATE INDEX idx_projects_public ON public.projects(is_public);
CREATE INDEX idx_projects_author ON public.projects(author);
