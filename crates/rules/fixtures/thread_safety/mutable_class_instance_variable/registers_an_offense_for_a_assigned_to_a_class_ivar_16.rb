# frozen_string_literal: true
class Test
  @var ||= "#{a}"
           ^^^^^^ Freeze mutable objects assigned to class instance variables.
end