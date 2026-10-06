class Account < ApplicationRecord
  with_options dependent: :destroy do |assoc|
    assoc.has_many :customers
    ^^^^^ Redundant receiver in `with_options`.
    assoc.has_many :products
    ^^^^^ Redundant receiver in `with_options`.
    assoc.has_many :invoices
    ^^^^^ Redundant receiver in `with_options`.
    assoc.has_many :expenses
    ^^^^^ Redundant receiver in `with_options`.
  end
end
