# frozen_string_literal: true

class Method
  def <<(other)
    ->(*args, &block) { call(other.call(*args, &block)) }
  end

  def >>(other)
    ->(*args, &block) { other.call(call(*args, &block)) }
  end

  def to_proc
    m = self
    ->(*args, &b) { m.call(*args, &b) }
  end
end
