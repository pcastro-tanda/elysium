sig { returns(T::Array[String]) }
def has_caused_error
  errors.add(:base, 'This has caused an error')
end
