class User < ApplicationRecord
  after_save :after_save_callback1
  after_save :after_save_callback2
  after_commit :after_commit_callback
end
