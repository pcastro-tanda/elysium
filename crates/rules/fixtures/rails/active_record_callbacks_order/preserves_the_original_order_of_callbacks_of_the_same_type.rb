class User < ApplicationRecord
  after_commit :after_commit_callback
  after_save :after_save_callback1
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `after_save` is supposed to appear before `after_commit`.
  after_save :after_save_callback2
end
