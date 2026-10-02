class Email < ApplicationRecord
  validates :address, presence: true, uniqueness: { case_sensitive: false }, email: true
end
