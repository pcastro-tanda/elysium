each {
  case state
  in 0
    case name
    in :a
    end
  in 1
    loop {
      case name
      in :b
      end
    }
  end
}
case s
in Array
end
