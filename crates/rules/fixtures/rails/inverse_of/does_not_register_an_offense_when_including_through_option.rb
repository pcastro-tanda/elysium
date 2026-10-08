class Physician < ApplicationRecord
  has_many :appointments
  has_many :patients, -> () { where(bar: true) }, through: :appointments
end
