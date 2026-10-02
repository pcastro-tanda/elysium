class Post < ApplicationRecord
  belongs_to :foos, -> { active }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
  alias bars foos
  belongs_to :foos
  ^^^^^^^^^^^^^^^^ Association `foos` is defined multiple times. Don't repeat associations.
end
