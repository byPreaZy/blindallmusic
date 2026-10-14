-- Facebook OAuth login is gone; the Meta data-deletion callback flow is
-- retired too (requests table had no non-test rows outside the old flow).
DROP TABLE IF EXISTS facebook_data_deletion_requests;
