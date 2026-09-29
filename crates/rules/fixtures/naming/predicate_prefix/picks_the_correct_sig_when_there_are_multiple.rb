sig { returns(T::Boolean) }
def is_attr?; end

sig { returns(String) }
def has_caused_error
  errors.add(:base, 'This has caused an error')
end
