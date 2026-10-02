class User < ApplicationRecord
  after_create_commit :log_create_action
  after_destroy_commit :log_destroy_action
end
