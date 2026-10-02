class User < ApplicationRecord
  after_create_commit :log_action
  after_commit :log_action, on: :update
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ There can only be one `after_*_commit :log_action` hook defined for a model.
  after_destroy_commit :log_action
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ There can only be one `after_*_commit :log_action` hook defined for a model.
end
