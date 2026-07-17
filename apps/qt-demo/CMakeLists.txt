cmake_minimum_required(VERSION 3.16)
project(cheetah-qt-demo LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt5 REQUIRED COMPONENTS Widgets Test)
find_package(Threads REQUIRED)

set(CBINDINGS_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../crates/cheetah-media-c-bindings)
set(CBINDINGS_HEADER ${CBINDINGS_DIR}/include/cheetah_media.h)

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(CARGO_PROFILE "")
  set(CBINDINGS_SO ${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libcheetah_media_c_bindings.so)
else()
  set(CARGO_PROFILE "--release")
  set(CBINDINGS_SO ${CMAKE_CURRENT_SOURCE_DIR}/../../target/release/libcheetah_media_c_bindings.so)
endif()

# Always invoke cargo so it can do its own incremental up-to-date checks.
# Depending only on lib.rs/header would miss transitive crate changes.
add_custom_target(cheetah_c_bindings ALL
  COMMAND cargo build -p cheetah-media-c-bindings ${CARGO_PROFILE}
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/../..
  COMMENT "Building cheetah-media-c-bindings (${CBINDINGS_SO})"
  VERBATIM
)

# Demo executable
add_executable(cheetah-qt-demo
  src/main.cpp
  src/mainwindow.cpp
  src/mainwindow.h
  src/cheetah_player_widget.cpp
  src/cheetah_player_widget.h
)

add_dependencies(cheetah-qt-demo cheetah_c_bindings)

target_include_directories(cheetah-qt-demo PRIVATE ${CBINDINGS_DIR}/include)
target_link_libraries(cheetah-qt-demo PRIVATE ${CBINDINGS_SO} Qt5::Widgets Threads::Threads)

add_custom_command(TARGET cheetah-qt-demo POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CBINDINGS_SO} $<TARGET_FILE_DIR:cheetah-qt-demo>
)

set_target_properties(cheetah-qt-demo PROPERTIES
  BUILD_RPATH "$ORIGIN"
  INSTALL_RPATH "$ORIGIN"
)

# QTest smoke test
add_executable(cheetah-qt-demo-test
  tests/test_cheetah_player_widget.cpp
  tests/test_cheetah_player_widget.h
  src/cheetah_player_widget.cpp
  src/cheetah_player_widget.h
)

add_dependencies(cheetah-qt-demo-test cheetah_c_bindings)

target_include_directories(cheetah-qt-demo-test PRIVATE ${CBINDINGS_DIR}/include)
target_link_libraries(cheetah-qt-demo-test PRIVATE ${CBINDINGS_SO} Qt5::Widgets Qt5::Test Threads::Threads)

add_custom_command(TARGET cheetah-qt-demo-test POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CBINDINGS_SO} $<TARGET_FILE_DIR:cheetah-qt-demo-test>
)

set_target_properties(cheetah-qt-demo-test PROPERTIES
  BUILD_RPATH "$ORIGIN"
  INSTALL_RPATH "$ORIGIN"
)

enable_testing()
add_test(NAME cheetah-qt-demo-smoke
  COMMAND cheetah-qt-demo-test
)
set_tests_properties(cheetah-qt-demo-smoke PROPERTIES
  ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
)
