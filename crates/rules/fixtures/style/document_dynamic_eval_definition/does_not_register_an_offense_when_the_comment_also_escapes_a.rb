class_eval(
  # def something
  #   puts "escaped \#{interpolation}"
  # end

  <<~EOT, __FILE__, __LINE__ + 1
    def #{my_method}
      puts "escaped \#{interpolation}"
    end
  EOT
)
