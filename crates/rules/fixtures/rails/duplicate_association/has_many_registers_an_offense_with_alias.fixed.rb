class Post < ApplicationRecord
  belongs_to :foos
  alias bars foos
end
