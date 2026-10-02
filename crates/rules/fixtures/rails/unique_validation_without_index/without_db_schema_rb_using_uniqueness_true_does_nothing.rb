class User < ApplicationRecord
  validates :account, uniqueness: true
end
