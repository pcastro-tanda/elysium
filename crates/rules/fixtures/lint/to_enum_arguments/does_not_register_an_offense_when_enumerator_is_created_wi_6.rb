def do_something(**)
  return to_enum(:do_something, **) unless block_given?

  do_something_else
end
