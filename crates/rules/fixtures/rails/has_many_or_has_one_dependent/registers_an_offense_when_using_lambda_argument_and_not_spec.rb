class User < ApplicationRecord
  has_many :articles, -> { where(active: true) }
  ^^^^^^^^ Specify a `:dependent` option.
end
