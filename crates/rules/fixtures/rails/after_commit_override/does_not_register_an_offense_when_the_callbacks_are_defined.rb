class User < ApplicationRecord
  after_commit -> { foo }
  after_destroy_commit -> { foo }
end
