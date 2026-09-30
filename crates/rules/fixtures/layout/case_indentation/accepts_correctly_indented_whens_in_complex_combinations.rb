each {
  case state
  when 0
    case name
    when :a
    end
  when 1
    loop {
      case name
      when :b
      end
    }
  end
}
case s
when Array
end
