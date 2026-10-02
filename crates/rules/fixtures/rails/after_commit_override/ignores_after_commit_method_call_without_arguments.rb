class User < ApplicationRecord
  after_commit
  after_destroy_commit
end
