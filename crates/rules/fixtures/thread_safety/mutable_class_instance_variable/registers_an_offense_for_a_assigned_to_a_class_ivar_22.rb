# frozen_string_literal: true
module Test
  @var ||= "#{a}"
           ^^^^^^ Freeze mutable objects assigned to class instance variables.
end