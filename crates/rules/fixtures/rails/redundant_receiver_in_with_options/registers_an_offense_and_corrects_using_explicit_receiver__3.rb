class Account < ApplicationRecord
  with_options dependent: :destroy do
    it.has_many :customers
    ^^ Redundant receiver in `with_options`.
    it.has_many :products
    ^^ Redundant receiver in `with_options`.
    it.has_many :invoices
    ^^ Redundant receiver in `with_options`.
    it.has_many :expenses
    ^^ Redundant receiver in `with_options`.
  end
end
