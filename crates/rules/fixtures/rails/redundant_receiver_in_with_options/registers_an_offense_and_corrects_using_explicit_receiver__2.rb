class Account < ApplicationRecord
  with_options dependent: :destroy do
    _1.has_many :customers
    ^^ Redundant receiver in `with_options`.
    _1.has_many :products
    ^^ Redundant receiver in `with_options`.
    _1.has_many :invoices
    ^^ Redundant receiver in `with_options`.
    _1.has_many :expenses
    ^^ Redundant receiver in `with_options`.
  end
end
