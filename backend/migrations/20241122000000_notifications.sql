-- 5. Notifications Table
CREATE TABLE IF NOT EXISTS public.notifications (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE, -- Recipient (Project Owner)
    actor_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE, -- Trigger-er (The Liker/Commenter)
    project_id UUID NOT NULL REFERENCES public.projects(id) ON DELETE CASCADE,
    type TEXT NOT NULL CHECK (type IN ('like', 'comment')),
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fast lookup (e.g., "Get my unread notifications")
CREATE INDEX idx_notifications_user_unread ON public.notifications(user_id) WHERE is_read = false;

-- ==========================================
-- LOGIC: Database Triggers
-- ==========================================

-- Trigger Function: Handle Likes
CREATE OR REPLACE FUNCTION public.handle_new_like()
RETURNS TRIGGER AS $$
DECLARE
    project_owner_id UUID;
BEGIN
    -- 1. Find who owns the project being liked
    SELECT user_id INTO project_owner_id
    FROM public.projects
    WHERE id = NEW.project_id;

    -- 2. If project doesn't exist or User likes their own project, do nothing
    IF project_owner_id IS NULL OR project_owner_id = NEW.user_id THEN
        RETURN NEW;
    END IF;

    -- 3. Insert Notification
    INSERT INTO public.notifications (user_id, actor_id, project_id, type)
    VALUES (project_owner_id, NEW.user_id, NEW.project_id, 'like')
    ON CONFLICT DO NOTHING; -- Safety valve

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Attach to 'project_likes'
DROP TRIGGER IF EXISTS on_project_like ON public.project_likes;
CREATE TRIGGER on_project_like
    AFTER INSERT ON public.project_likes
    FOR EACH ROW EXECUTE FUNCTION public.handle_new_like();


-- Trigger Function: Handle Comments
CREATE OR REPLACE FUNCTION public.handle_new_comment()
RETURNS TRIGGER AS $$
DECLARE
    project_owner_id UUID;
BEGIN
    -- 1. Find who owns the project
    SELECT user_id INTO project_owner_id
    FROM public.projects
    WHERE id = NEW.project_id;

    -- 2. If User comments on their own project, do nothing
    IF project_owner_id IS NULL OR project_owner_id = NEW.user_id THEN
        RETURN NEW;
    END IF;

    -- 3. Insert Notification
    INSERT INTO public.notifications (user_id, actor_id, project_id, type)
    VALUES (project_owner_id, NEW.user_id, NEW.project_id, 'comment');

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Attach to 'project_comments'
DROP TRIGGER IF EXISTS on_project_comment ON public.project_comments;
CREATE TRIGGER on_project_comment
    AFTER INSERT ON public.project_comments
    FOR EACH ROW EXECUTE FUNCTION public.handle_new_comment();
