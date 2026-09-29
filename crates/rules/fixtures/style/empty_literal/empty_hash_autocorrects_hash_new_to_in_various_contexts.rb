test = Hash.new
       ^^^^^^^^ Use hash literal `{}` instead of `Hash.new`.
Hash.new.merge("a" => 3)
^^^^^^^^ Use hash literal `{}` instead of `Hash.new`.
yadayada.map { a }.reduce(Hash.new, :merge)
                          ^^^^^^^^ Use hash literal `{}` instead of `Hash.new`.
