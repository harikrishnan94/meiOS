#pragma once

#include <ktl/error.hpp>
#include <ktl/expected.hpp>

namespace mei {
template<typename T>
using Result = ktl::expected<T, ktl::Error>;
}