class Email < ApplicationRecord
  validates :address, presence: true, uniqueness: { case_sensitive: false }, email: true
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
